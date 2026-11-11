//! Shows what the desktop app observes: output volume and mute, microphone
//! mute, media players. Read-only. With `--watch`, a line is printed only
//! when something actually changed (the sound server sends several identical
//! notifications for one change).
//!
//! ```sh
//! cargo run -p padboard-actions --example state            # once
//! cargo run -p padboard-actions --example state -- --watch # on every change
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use padboard_actions::state::{self, AudioChanges};

fn describe() -> String {
    let audio = match state::audio() {
        Ok(a) => format!(
            "output {} %{} · microphone {}",
            a.output_percent,
            if a.output_muted { " (muted)" } else { "" },
            match a.input_muted {
                None => "none",
                Some(true) => "muted",
                Some(false) => "on",
            }
        ),
        Err(e) => format!("audio: {e}"),
    };
    let media = match state::media_playing() {
        Ok(None) => "media: no player".to_owned(),
        Ok(Some(playing)) => format!("media: {}", if playing { "playing" } else { "not playing" }),
        Err(e) => format!("media: {e}"),
    };
    format!("{audio}\n{media}")
}

fn main() -> ExitCode {
    let mut shown = describe();
    println!("{shown}");
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
        let now = describe();
        if now != shown {
            println!("{now}");
            shown = now;
        }
    }
    eprintln!("the sound server went away");
    ExitCode::FAILURE
}
