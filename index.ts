// Create a serial USB listener, which emulates keyboard presses, when receiving data over USB serial.
// Use vanilla ts
import { SerialPort } from 'serialport';
import { ReadlineParser } from '@serialport/parser-readline';
import koffi from 'koffi';

const lib = koffi.load('./native/target/release/native.dll');
const send_key_down = lib.func('send_key_down', 'void', ['uint16']);
const send_key_up = lib.func('send_key_up', 'void', ['uint16']);

const keyMap: { [key: string]: number } = {
    'left': 0x25,
    'up': 0x26,
    'right': 0x27,
    'down': 0x28,
    'pageup': 0x21,
    'pagedown': 0x22,
};

// Detect whether we are running on Windows or Linux
const isWindows = process.platform === 'win32';
const isLinux = process.platform === 'linux';

console.log(`Running on platform: ${process.platform}`);

if (!isWindows && !isLinux) {
    console.error('This script only supports Windows and Linux platforms.');
    process.exit(1);
}

// Determine the appropriate serial port path based on the platform
let serialPortPath: string;

if (isWindows) {
    serialPortPath = 'COM6'; // Adjust this to your Windows COM port
} else if (isLinux) {
    serialPortPath = '/dev/ttyACM0'; // Adjust this to your Linux device path
} else {
    throw new Error('Unsupported platform');
}

/*
// List available ports
SerialPort.list().then((ports) => {
    console.log('Available serial ports:');
    ports.forEach((port) => {
        console.log(`${port.path} - ${port.manufacturer || 'Unknown'}`);
    });
}).catch((err) => {
    console.error('Error listing ports:', err.message);
});
*/

const port = new SerialPort({ path: serialPortPath, baudRate: 57600 });

const parser = port.pipe(new ReadlineParser({ delimiter: '\n' }));

parser.on('data', (line: string) => {
    console.log(`Received data: ${line}`);
    // Parse the G-code command and emulate keyboard input for Mach3
    let command = line.trim();
    if (command.startsWith('GCODE: ')) {
        command = command.substring(7);
    }
    // Assuming format: G91G0<axis><value> where axis is X, Y, or Z
    const match = command.match(/G91G0([XYZ])(-?\d+\.?\d*)/);
    if (match) {
        const axis = match[1];
        const value = parseFloat(match[2]);
        let key: string;
        if (axis === 'Y') {
            key = value > 0 ? 'up' : 'down';
        } else if (axis === 'X') {
            key = value > 0 ? 'right' : 'left';
        } else if (axis === 'Z') {
            key = value > 0 ? 'pageup' : 'pagedown';
        } else {
            console.log('Unsupported axis:', axis);
            return;
        }
        const vk = keyMap[key];
        if (vk) {
            send_key_down(0x11); // VK_CONTROL
            send_key_down(vk);
            send_key_up(vk);
            send_key_up(0x11);
            console.log(`Simulated Ctrl + ${key} for ${axis} axis`);
        } else {
            console.log('Unsupported key:', key);
        }
    } else {
        console.log('Unrecognized command:', command);
    }
});

port.on('open', () => {
    console.log(`Serial port ${serialPortPath} opened at 115200 baud rate.`);
});

port.on('close', () => {
    console.log('Serial port closed.');
});

port.on('error', (err) => {
    console.error(`Error: ${err.message}`);
});

// Keep the process alive to listen for serial data
setInterval(() => {
    // Prevent the process from exiting
}, 1000);