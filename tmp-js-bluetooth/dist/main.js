"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
// The following code is inspired by https://create.arduino.cc/projecthub/leevinentwilson/bluetooth-node-and-arduino-de822e
const csv_append_1 = __importDefault(require("csv-append"));
const stream_1 = require("stream");
const node_microphone_1 = __importDefault(require("node-microphone"));
const BTSerialPort = __importStar(require("bluetooth-serial-port"));
const mic_1 = require("./mic");
let mic = new node_microphone_1.default();
let micStream = new stream_1.Writable();
let mostRecentDb = 0;
micStream.on("data", (chunk) => console.log("GOT CHUNK", chunk));
const CSV_PATH = "./data.csv";
const { append, end } = (0, csv_append_1.default)(CSV_PATH);
const btSerial = new BTSerialPort.BluetoothSerialPort();
const errFunction = (err) => {
    if (err) {
        console.log("Error", err);
    }
};
const connect = (dataCb) => {
    return new Promise((res, rej) => {
        // Once BtSerial.inquire finds a device it will call this code
        // BtSerial.inquire will find all devices currently connected with your computer
        btSerial.on("found", (address, name) => __awaiter(void 0, void 0, void 0, function* () {
            // If a device is found and the name contains 'HC' we will continue
            // This is so that it doesn't try to send data to all your other connected BT devices
            if (name === "HC-05" || name === "00:21:09:01:35:D7") {
                console.log("Found BT module with name", name, "and address", address);
                btSerial.findSerialPortChannel(address, (channel) => {
                    console.log("Found channel:", channel);
                    // Finds then serial port channel and then connects to it
                    btSerial.connect(address, channel, () => {
                        // Now the magic begins, bTSerial.on('data', callbackFunc) listens to the bluetooth device.
                        // If any data is received from it the call back function is used
                        btSerial.on("data", (b) => dataCb(Buffer.from(b).toString()));
                        res(btSerial);
                    }, errFunction);
                }, 
                //@ts-ignore
                errFunction);
            }
            else {
                console.log("Not connecting to: ", name);
            }
        }));
        btSerial.inquire();
    });
};
const callBackData = (data) => __awaiter(void 0, void 0, void 0, function* () {
    console.log("received", data);
    if (data.split(",").length >= 2) {
        const [aX, aY, aZ, hr] = data.split("\n")[0].split(",");
        const time = Date.now();
        console.log("AAAAA", hr);
        append({
            time,
            aX,
            aY,
            aZ,
            hr: parseInt(hr.replace("e", "")),
            soundDB: mostRecentDb,
        });
    }
    // await end();
});
function main() {
    return __awaiter(this, void 0, void 0, function* () {
        (0, mic_1.startRecord)((level) => {
            const db = 20 * Math.log10(level / 100);
            mostRecentDb = db;
        });
        // await connectMic();
        const btConn = yield connect(callBackData);
        console.log("Connected");
        btConn.write(Buffer.from("From Node With Love\n"), errFunction);
    });
}
main();
// Tmrw
// get all data and log as CSV
// have Electron launch the process
//# sourceMappingURL=main.js.map