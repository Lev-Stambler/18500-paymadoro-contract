"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.startRecord = void 0;
const { spawn } = require("child_process");
const arOptions = {
    channels: 2,
    rate: 16000,
    format: "S16_LE",
    device: "default", // find out with `arecord -L`
};
const startRecord = (cb) => {
    const arProcess = spawn("arecord", [
        "-c",
        arOptions.channels,
        "-r",
        arOptions.rate,
        "-f",
        arOptions.format,
        "-D",
        arOptions.device,
        "-V",
        "mono",
    ], { stdio: ["ignore", "ignore", "pipe"] });
    arProcess.stderr.on("data", function (data) {
        let level = parseInt(String(data).substr(54, 2));
        if (isNaN(level)) {
            console.log(String(data));
            return;
        }
        cb(level);
    });
};
exports.startRecord = startRecord;
//# sourceMappingURL=mic.js.map