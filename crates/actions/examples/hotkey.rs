//! Manual test of keyboard injection on this machine.
//!
//! ```sh
//! cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT
//! ```
//!
//! Keys are W3C `KeyboardEvent.code` names. The shortcut is sent after a short
//! countdown so you can focus the target window.

use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use padboard_actions::{Action, Hotkey, Key, platform_executor};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: hotkey <KeyCode>... (e.g. ControlLeft AltLeft KeyT)");
        return ExitCode::from(2);
    }
    let mut keys = Vec::new();
    for arg in &args {
        let Some(key) = Key::from_code(arg) else {
            eprintln!("unknown key code: {arg} (expected a W3C KeyboardEvent.code name)");
            return ExitCode::from(2);
        };
        keys.push(key);
    }
    let hotkey = Hotkey::new(keys);

    let executor = platform_executor();
    for n in (1..=3).rev() {
        println!("Sending {hotkey} in {n}…");
        thread::sleep(Duration::from_secs(1));
    }
    match executor.execute(&Action::Hotkey(hotkey)) {
        Ok(()) => {
            println!("Sent.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
