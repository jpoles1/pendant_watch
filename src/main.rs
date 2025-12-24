//! # Pendant Watch
//!
//! A serial-based pendant controller for CNC machines and CAD software.
//! This application connects to a serial device (like an Arduino pendant) and translates
//! GCODE movement commands into keyboard inputs for controlling software applications.
//!
//! ## Features
//!
//! - **Arrow Mode**: Translates GCODE G91G0 commands into arrow key presses with Ctrl modifier
//! - **Gcode Mode**: Allows manual typing and sending of GCODE commands to the device
//! - Real-time serial communication with configurable port and baud rate
//! - Terminal-based UI with status display
//!
//! ## Usage
//!
//! Run the application and use:
//! - '1' to switch to Arrow Mode
//! - '2' to switch to Gcode Mode
//! - 'q' to quit
//!
//! In Gcode Mode, type commands and press Enter to send them.

use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};
use regex::Regex;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};

/// Operating modes for the pendant controller
#[derive(PartialEq)]
enum Mode {
    /// Arrow mode: Translates GCODE movements to keyboard arrow keys
    Arrow,
    /// Gcode mode: Allows manual GCODE input and transmission
    Gcode,
}

/// Application state containing current mode, connection status, and command history
struct AppState {
    mode: Mode,
    connected: bool,
    last_command: Option<String>,
    last_command_time: Option<Instant>,
    gcode_input: String,
}

impl AppState {
    /// Creates a new application state with default values
    fn new() -> Self {
        Self {
            mode: Mode::Gcode,
            connected: false,
            last_command: None,
            last_command_time: None,
            gcode_input: String::new(),
        }
    }

    /// Updates the last command and its timestamp
    fn update_last_command(&mut self, command: String) {
        self.last_command = Some(command);
        self.last_command_time = Some(Instant::now());
    }

    /// Returns the duration since the last command was processed, if any
    fn time_since_last_command(&self) -> Option<Duration> {
        self.last_command_time.map(|t| t.elapsed())
    }
}

/// Simulates a key press down event using Windows API
/// # Safety
/// This function uses unsafe Windows API calls
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

/// Simulates a key release event using Windows API
/// # Safety
/// This function uses unsafe Windows API calls
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

/// Types out text by simulating individual key presses and releases
/// # Safety
/// This function uses unsafe Windows API calls for each character
fn type_text(text: &str) {
    for ch in text.chars() {
        let input_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: ch as u16,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                }
            }
        };
        unsafe {
            SendInput(&[input_down], std::mem::size_of::<INPUT>() as i32);
        }

        let input_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: ch as u16,
                    dwFlags: KEYEVENTF_KEYUP | KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                }
            }
        };
        unsafe {
            SendInput(&[input_up], std::mem::size_of::<INPUT>() as i32);
        }
    }
}

/// Draws the terminal-based status bar and instructions
fn draw_status_bar(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    // Clear screen and move cursor to top-left
    print!("\x1B[2J\x1B[1;1H");

    // Draw status bar border
    println!("┌─────────────────────────────────────────────────────────────────────────────────┐");

    // Display connection status
    print!("│ Connected: {} │ ", if state.connected { "Yes" } else { "No" });

    // Display last command (truncated if too long)
    if let Some(ref cmd) = state.last_command {
        let truncated_cmd = if cmd.len() > 20 {
            format!("{}...", &cmd[..17])
        } else {
            cmd.clone()
        };
        print!("Last: {} │ ", truncated_cmd);
    } else {
        print!("Last: None │ ");
    }

    // Display time since last command
    if let Some(duration) = state.time_since_last_command() {
        let secs = duration.as_secs();
        if secs < 60 {
            print!("Time: {}s │ ", secs);
        } else {
            let mins = secs / 60;
            print!("Time: {}m{}s │ ", mins, secs % 60);
        }
    } else {
        print!("Time: N/A │ ");
    }

    // Display current mode
    match state.mode {
        Mode::Arrow => println!("Mode: Arrow │"),
        Mode::Gcode => println!("Mode: Gcode │"),
    }

    // Close status bar border
    println!("└─────────────────────────────────────────────────────────────────────────────────┘");
    println!();

    // Display mode-specific instructions
    match state.mode {
        Mode::Arrow => {
            println!("Arrow Mode: Receiving commands from device and simulating keyboard presses.");
            println!("Press '1' for Arrow Mode, '2' for Gcode Mode, 'q' to quit.");
        }
        Mode::Gcode => {
            println!("Gcode Mode: Type GCODE commands and press Enter to send to device.");
            println!("Current input: {}", state.gcode_input);
            println!("Press '1' for Arrow Mode, '2' for Gcode Mode, 'q' to quit.");
        }
    }
    println!();

    io::stdout().flush()?;
    Ok(())
}

fn serial_to_gcode(line: &str, state: &mut AppState) {
    // In Gcode mode, type out received commands as text input
    type_text(line);
    // Press enter after typing the command
    // Simulate Enter key press
    send_key_down(0x0D); // VK_RETURN
    send_key_up(0x0D);  // VK_RETURN
    state.update_last_command(format!("Typed: {}", line));
}

/// Virtual key codes for arrow keys and page keys
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_PAGEUP: u16 = 0x21;
const VK_PAGEDOWN: u16 = 0x22;
const VK_CONTROL: u16 = 0x11;

/// Processes incoming serial data and converts GCODE movement commands to keyboard input
/// Returns true if a command was processed successfully
/// Processes incoming serial data and converts GCODE movement commands to keyboard input
/// Returns true if a command was processed successfully
fn serial_to_arrow(line: &str, state: &mut AppState) -> bool {
    // Regex to match G91G0 commands with axis and value: G91G0X10.5, G91G0Y-5, etc.
    let re = Regex::new(r"G91G0([XYZ])(-?\d+\.?\d*)").unwrap();

    // Remove "GCODE: " prefix if present
    let mut command = line.trim();
    if command.starts_with("GCODE: ") {
        command = &command[7..];
    }

    // Update command history
    state.update_last_command(command.to_string());

    // Try to match and process the command
    if let Some(captures) = re.captures(command) {
        // Extract axis and movement value
        let axis = &captures[1];
        let value: f32 = captures[2].parse().unwrap();

        // Determine which key to press based on axis and direction
        let key = match axis {
            "Y" => if value > 0.0 { "up" } else { "down" },
            "X" => if value > 0.0 { "right" } else { "left" },
            "Z" => if value > 0.0 { "pageup" } else { "pagedown" },
            _ => {
                return false; // Invalid axis
            }
        };

        // Map key name to virtual key code
        let vk = match key {
            "left" => VK_LEFT,
            "up" => VK_UP,
            "right" => VK_RIGHT,
            "down" => VK_DOWN,
            "pageup" => VK_PAGEUP,
            "pagedown" => VK_PAGEDOWN,
            _ => return false,
        };

        // Simulate Ctrl+key combination (common in CAD software for jogging)
        send_key_down(VK_CONTROL);
        send_key_down(vk);
        send_key_up(vk);
        send_key_up(VK_CONTROL);

        true
    } else {
        false // No matching command found
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration constants
    let port_name = "COM6";
    let baud_rate = 115200;

    // Attempt to open serial port
    let port = match serialport::new(port_name, baud_rate)
        .timeout(std::time::Duration::from_millis(10))
        .open() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to open serial port {}: {}", port_name, e);
            return Ok(());
        }
    };

    println!("Serial port {} opened at {} baud rate.", port_name, baud_rate);

    // Create reader and writer for the serial port
    let mut reader = io::BufReader::new(port.try_clone()?);
    let mut writer = port;

    // Initialize application state
    let mut state = AppState::new();
    state.connected = true;

    // Setup terminal for raw mode input handling
    enable_raw_mode()?;
    // execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    // Draw initial status bar
    draw_status_bar(&state)?;

    // Main event loop
    run_event_loop(&mut reader, &mut writer, &mut state)?;

    // Restore terminal settings
    disable_raw_mode()?;
    // execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}

/// Runs the main event loop handling serial data and keyboard input
fn run_event_loop(
    reader: &mut io::BufReader<Box<dyn serialport::SerialPort>>,
    writer: &mut Box<dyn serialport::SerialPort>,
    state: &mut AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Check for incoming serial data
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => {
                let line = line.trim();
                match state.mode {
                    Mode::Arrow => {
                        serial_to_arrow(line, state);
                    }
                    Mode::Gcode => {
                        serial_to_gcode(line, state);
                    }
                }
                draw_status_bar(state)?;
            }
            _ => {} // No data available or timeout
        }

        // Check for keyboard input
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                handle_key_press(key, writer, state)?;
                if state.mode == Mode::Gcode {
                    draw_status_bar(state)?;
                }
            }
        }
    }
}

/// Handles keyboard input based on current mode
fn handle_key_press(
    key: event::KeyEvent,
    writer: &mut Box<dyn serialport::SerialPort>,
    state: &mut AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Char('1') => {
            state.mode = Mode::Arrow;
            draw_status_bar(state)?;
        }
        KeyCode::Char('2') => {
            state.mode = Mode::Gcode;
            draw_status_bar(state)?;
        }
        KeyCode::Char('q') => {
            // Quit the application
            std::process::exit(0);
        }
        KeyCode::Enter if matches!(state.mode, Mode::Gcode) => {
            if !state.gcode_input.is_empty() {
                // Send GCODE command to device
                let gcode = format!("{}\n", state.gcode_input);
                writer.write_all(gcode.as_bytes())?;
                writer.flush()?;
                state.update_last_command(format!("Sent: {}", state.gcode_input));
                state.gcode_input.clear();
                draw_status_bar(state)?;
            }
        }
        KeyCode::Backspace if matches!(state.mode, Mode::Gcode) => {
            state.gcode_input.pop();
            draw_status_bar(state)?;
        }
        KeyCode::Char(c) if matches!(state.mode, Mode::Gcode) => {
            state.gcode_input.push(c);
            draw_status_bar(state)?;
        }
        _ => {}
    }
    Ok(())
}