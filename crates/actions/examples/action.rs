//! Runs one action on this machine, as a button press would.
//!
//! ```sh
//! cargo run -p padboard-actions --example action -- '{"type":"microphone","params":{"command":"toggle_mute"}}'
//! cargo run -p padboard-actions --example action -- '{"type":"volume","params":{"command":"up"}}'
//! cargo run -p padboard-actions --example action -- '{"type":"media","params":{"command":"play_pause"}}'
//! cargo run -p padboard-actions --example action -- '{"type":"open","params":{"target":"https://example.org"}}'
//! cargo run -p padboard-actions --example action -- '{"type":"lock"}'
//! ```

#![allow(clippy::print_stderr)]

use std::process::ExitCode;

use padboard_actions::{Action, platform_executor};

fn main() -> ExitCode {
    let Some(json) = std::env::args().nth(1) else {
        eprintln!("usage: action '<action JSON>' (see the examples at the top of this file)");
        return ExitCode::from(2);
    };
    let action: Action = match serde_json::from_str(&json) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("not an action: {e}");
            return ExitCode::from(2);
        }
    };
    match platform_executor().execute(&action) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
