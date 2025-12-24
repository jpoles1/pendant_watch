use std::io::{self, BufRead};
use regex::Regex;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

fn send_key_down(key_code: u16) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(key_code),
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            }
        }
    };

    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_key_up(key_code: u16) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(key_code),
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            }
        }
    };

    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn main() {
    let port_name = "COM6";
    let baud_rate = 57600;

    let port = serialport::new(port_name, baud_rate)
        .open()
        .expect("Failed to open serial port");

    println!("Serial port {} opened at {} baud rate.", port_name, baud_rate);

    let mut reader = io::BufReader::new(port);

    let re = Regex::new(r"G91G0([XYZ])(-?\d+\.?\d*)").unwrap();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let line = line.trim();
                println!("Received data: {}", line);
                let mut command = line;
                if command.starts_with("GCODE: ") {
                    command = &command[7..];
                }
                if let Some(captures) = re.captures(command) {
                    let axis = &captures[1];
                    let value: f32 = captures[2].parse().unwrap();
                    let key = match axis {
                        "Y" => if value > 0.0 { "up" } else { "down" },
                        "X" => if value > 0.0 { "right" } else { "left" },
                        "Z" => if value > 0.0 { "pageup" } else { "pagedown" },
                        _ => {
                            println!("Unsupported axis: {}", axis);
                            continue;
                        }
                    };
                    let vk = match key {
                        "left" => 0x25,
                        "up" => 0x26,
                        "right" => 0x27,
                        "down" => 0x28,
                        "pageup" => 0x21,
                        "pagedown" => 0x22,
                        _ => continue,
                    };
                    send_key_down(0x11); // VK_CONTROL
                    send_key_down(vk);
                    send_key_up(vk);
                    send_key_up(0x11);
                    println!("Simulated Ctrl + {} for {} axis", key, axis);
                } else {
                    println!("Unrecognized command: {}", command);
                }
            }
            Err(e) => eprintln!("Error reading: {}", e),
        }
    }
}