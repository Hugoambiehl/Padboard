//! Shows what the desktop app observes: output volume and mute, microphone
//! mute, media players. Read-only.
//!
//! ```sh
//! cargo run -p padboard-actions --example state            # once
//! cargo run -p padboard-actions --example state -- --watch # on every change
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use padboard_actions::state::{self, AudioChanges};

fn show() {
    match state::audio() {
        Ok(a) => println!(
            "output {} %{} · microphone {}",
            a.output_percent,
            if a.output_muted { " (muted)" } else { "" },
            match a.input_muted {
                None => "none",
                Some(true) => "muted",
                Some(false) => "on",
            }
        ),
        Err(e) => println!("audio: {e}"),
    }
    match state::media_playing() {
        Ok(None) => println!("media: no player"),
        Ok(Some(playing)) => println!("media: {}", if playing { "playing" } else { "not playing" }),
        Err(e) => println!("media: {e}"),
    }
}

fn main() -> ExitCode {
    show();
    if std::env::args().nth(1).as_deref() != Some("--watch") {
        return ExitCode::SUCCESS;
    }
    let mut changes = match AudioChanges::subscribe() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Watching (Ctrl+C to quit)…");
    while changes.wait().is_ok() {
        show();
    }
    eprintln!("the sound server went away");
    ExitCode::FAILURE
}
