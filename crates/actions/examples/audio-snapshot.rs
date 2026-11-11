//! Saves and restores the exact state of the default audio devices (raw
//! volume of each channel, mute), for tests that change them (Linux).
//!
//! ```sh
//! cargo run -p padboard-actions --example audio-snapshot -- save /tmp/audio.json
//! cargo run -p padboard-actions --example audio-snapshot -- restore /tmp/audio.json
//! cargo run -p padboard-actions --example audio-snapshot -- show
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

#[cfg(target_os = "linux")]
fn run(args: &[String]) -> Result<(), String> {
    use padboard_actions::state::snapshot;
    match args {
        [cmd, file] if cmd == "save" => {
            let s = snapshot::take().map_err(|e| e.to_string())?;
            std::fs::write(
                file,
                serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            println!("saved: {s:?}");
            Ok(())
        }
        [cmd, file] if cmd == "restore" => {
            let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
            let s: snapshot::RawSnapshot =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            snapshot::restore(&s).map_err(|e| e.to_string())?;
            let now = snapshot::take().map_err(|e| e.to_string())?;
            if now == s {
                println!("restored exactly");
                Ok(())
            } else {
                Err(format!("restored, but differs: {now:?}"))
            }
        }
        [cmd] if cmd == "show" => {
            println!("{:?}", snapshot::take().map_err(|e| e.to_string())?);
            Ok(())
        }
        _ => Err("usage: audio-snapshot save <file> | restore <file> | show".into()),
    }
}

#[cfg(not(target_os = "linux"))]
fn run(_: &[String]) -> Result<(), String> {
    Err("Linux only".into())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
