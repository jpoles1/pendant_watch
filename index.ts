// Create a serial USB listener, which emulates keyboard presses, when receiving data over USB serial.
// Use vanilla ts
import { SerialPort } from 'serialport';
import { ReadlineParser } from '@serialport/parser-readline';

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
    // Here you would add the logic to emulate keyboard presses based on the received data
    // This could involve using a library like 'robotjs' or similar to simulate key presses
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