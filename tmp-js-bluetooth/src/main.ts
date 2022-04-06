// The following code is inspired by https://create.arduino.cc/projecthub/leevinentwilson/bluetooth-node-and-arduino-de822e
import csvAppend from "csv-append";

import * as BTSerialPort from "bluetooth-serial-port";

const CSV_PATH = "./data.csv";
const { append, end } = csvAppend(CSV_PATH);

const btSerial = new BTSerialPort.BluetoothSerialPort();

const errFunction = (err) => {
  if (err) {
    console.log("Error", err);
  }
};

const connect = (
  dataCb: (string) => Promise<void>
): Promise<BTSerialPort.BluetoothSerialPort> => {
  return new Promise((res, rej) => {
    // Once BtSerial.inquire finds a device it will call this code
    // BtSerial.inquire will find all devices currently connected with your computer
    btSerial.on("found", async (address, name) => {
      // If a device is found and the name contains 'HC' we will continue
      // This is so that it doesn't try to send data to all your other connected BT devices
      if (name.toLowerCase().includes("hc")) {
        console.log("Found BT module with name", name, "and address", address);
        btSerial.findSerialPortChannel(
          address,
          (channel) => {
            console.log("Found channel:", channel);
            // Finds then serial port channel and then connects to it
            btSerial.connect(
              address,
              channel,
              () => {
                // Now the magic begins, bTSerial.on('data', callbackFunc) listens to the bluetooth device.
                // If any data is received from it the call back function is used
                btSerial.on("data", (b) => dataCb(Buffer.from(b).toString()));
                res(btSerial);
              },
              errFunction
            );
          },
          //@ts-ignore
          errFunction
        );
      } else {
        console.log("Not connecting to: ", name);
      }
    });
    btSerial.inquire();
  });
};

const callBackData = async (data: string) => {
  console.log("received", data);
  const [acceleration, hr] = data.split("\n")[0].split(",");
  const time = Date.now();

  append({
    time,
    acceleration,
    hr: hr.replace("e", ''),
  });
  // await end();
};


async function connectMic() {
}

async function main() {
  await connectMic()
  const btConn = await connect(callBackData);
  console.log("Connected");
  btConn.write(Buffer.from("From Node With Love\n"), errFunction);
}

main();
// Tmrw
// get all data and log as CSV
// have Electron launch the process
