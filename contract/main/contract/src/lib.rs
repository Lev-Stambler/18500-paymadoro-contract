/*!
Non-Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_contract_standards::non_fungible_token::{Token, TokenId};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::ValidAccountId;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, AccountId, Balance, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue,
};

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ActiveUsers {
    account: AccountId,
    time_start: u64,
    locked_amount: Balance,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    contract_confiscated_balance: Balance,
    active_users: Vec<ActiveUsers>,
    lock_amount: Balance,
    pomodoro_time_ms: u64,
    stale_time_ms: u64,
}

pub trait PaymadoroFN {
    /// Start a session
    fn start_session(&mut self);

    /// End a session
    fn end_session(&mut self, success: bool);

    /// Get all active users
    fn get_active_users(&self) -> Vec<AccountId>;

    /// Remove "stale" users who never ended there own pomodoro sessions.
    /// We consider a user "stale" once `self.stale_time_ms` elapsed
    fn prune_stale_users(&mut self);
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract owned by `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new(lock_amount: U128) -> Self {
        Self {
            contract_confiscated_balance: 0,
            active_users: vec![],
            lock_amount: lock_amount.0,
            // 25 minutes
            pomodoro_time_ms: 25 * 60 * 1_000,
            // 100 minutes
            stale_time_ms: 100 * 60 * 1_000,
        }
    }
}

impl Contract {
    fn remove_active_user(&mut self, active_user_ind: usize) {
        let refund_to = self.active_users[active_user_ind].account.clone();
        let storage_prior = env::storage_usage();
        self.active_users.remove(active_user_ind);
        let storage_post = env::storage_usage();

        let storage_refund = (storage_prior - storage_post) as u128 * env::storage_byte_cost();

        Promise::new(refund_to).transfer(storage_refund);
    }

    fn prune_stale_users_internal(&mut self) {
        let curr_time = env::block_timestamp();
        for i in (0..self.active_users.len()).rev() {
            if self.active_users[i].time_start + self.stale_time_ms < curr_time {
                self.remove_active_user(i)
            }
        }
    }

    fn check_attached_amount(&mut self, storage_change: u64, refund_to: AccountId) {
        let attached_deposit = env::attached_deposit();
        let storage_cost = storage_change as u128 * env::storage_byte_cost();
        let total_cost = storage_cost + self.lock_amount;
        assert!(
            attached_deposit >= total_cost,
            "Expected {} attached for the lockup and storage",
            total_cost
        );
        let refund = attached_deposit - total_cost;
        if refund != 0 {
            Promise::new(refund_to).transfer(refund);
        }
    }
}

#[near_bindgen]
impl PaymadoroFN for Contract {
    #[payable]
    fn start_session(&mut self) {
        self.prune_stale_users();

        let caller = env::predecessor_account_id();
        if self
            .active_users
            .iter()
            .find(|&user| &user.account == &caller)
            .is_some()
        {
            panic!("Cannot not start a session for a user that is currently active");
        }

        let user = ActiveUsers {
            account: caller.clone(),
            locked_amount: self.lock_amount,
            time_start: env::block_timestamp(),
        };
        let storage_incr = BorshSerialize::try_to_vec(&user).unwrap().len();
        self.active_users.push(user);
        self.check_attached_amount(storage_incr as u64, caller);
    }

    fn end_session(&mut self, success: bool) {
        let caller = env::current_account_id();
        let active_user = self.active_users.iter().position(|a| &a.account == &caller);
        match (active_user, success) {
            (None, _) => panic!("{} is not an active pomodoro user", caller),
            (Some(i), true) => {
                if self.pomodoro_time_ms + self.active_users[i].time_start > env::block_timestamp()
                {
                    panic!("Cannot end a session successfully in less than the Pomodoro period");
                }
                // Payout
                Promise::new(caller).transfer(self.contract_confiscated_balance);
                self.contract_confiscated_balance = 0;
                self.remove_active_user(i);
            }
            (Some(i), false) => {
                self.contract_confiscated_balance += self.lock_amount;
                self.remove_active_user(i);
            }
        }
    }

    fn get_active_users(&self) -> Vec<AccountId> {
        self.active_users
            .iter()
            .map(|a| a.account.clone())
            .collect()
    }

    fn prune_stale_users(&mut self) {
        self.prune_stale_users_internal()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    use super::*;

    const MIN_USER_STORAGE_COST: u128 = 5870000000000000000000;

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    fn sample_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Olympus Mons".into()),
            description: Some("The tallest mountain in the charted solar system".into()),
            media: None,
            media_hash: None,
            copies: Some(1u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new(128.into());
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.lock_amount, 128);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    #[should_panic]
    fn test_not_enough_attached() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let lock_amount = 128;
        let mut contract = Contract::new(lock_amount.into());
        let timestamp = 1_000;

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(127)
            .predecessor_account_id(accounts(0))
            .block_timestamp(timestamp)
            .build());

        contract.start_session();
    }

    #[test]
    fn test_start() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let lock_amount = 128;
        let mut contract = Contract::new(lock_amount.into());
        let timestamp = 1_000;

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MIN_USER_STORAGE_COST + lock_amount)
            .predecessor_account_id(accounts(0))
            .block_timestamp(timestamp)
            .build());

        contract.start_session();

        let user = contract
            .active_users
            .iter()
            .find(|a| &a.account == &accounts(0))
            .unwrap();

        assert_eq!(user.locked_amount, lock_amount);
        assert_eq!(user.time_start, timestamp);
    }

    #[test]
    fn test_end_fail_then_succeed() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let lock_amount = 128;
        let mut contract = Contract::new(lock_amount.into());
        let timestamp = 1_000;

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MIN_USER_STORAGE_COST + lock_amount)
            .predecessor_account_id(accounts(0))
            .block_timestamp(timestamp)
            .build());

        contract.start_session();

        let user = contract
            .active_users
            .iter()
            .find(|a| &a.account == &accounts(0))
            .unwrap();
        let lock_amount = user.locked_amount;
        let pot_bal_init = contract.contract_confiscated_balance;
        contract.end_session(false);
        let pot_bal_post = contract.contract_confiscated_balance;
        assert_eq!(pot_bal_post - pot_bal_init, lock_amount);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MIN_USER_STORAGE_COST + lock_amount)
            .predecessor_account_id(accounts(0))
            .block_timestamp(timestamp)
            .build());

        contract.start_session();
        let pot_bal_init = contract.contract_confiscated_balance;
        contract.end_session(true);
        let pot_bal_post = contract.contract_confiscated_balance;
        assert_eq!(pot_bal_init - pot_bal_post, lock_amount);
        assert_eq!(pot_bal_post, 0);
    }

    #[test]
    #[should_panic]
    fn test_panic_if_early_end_and_success() {
        todo!()
    }

    #[test]
    #[should_panic]
    fn test_panic_if_already_active() {
        todo!()
    }

    #[test]
    fn test_prune_stale_user() {
        todo!()
    }
}
