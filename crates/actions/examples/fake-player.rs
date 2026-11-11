//! A fake MPRIS media player on the session bus, to test media buttons and
//! the play/pause state without a real player (Linux).
//!
//! ```sh
//! cargo run -p padboard-actions --example fake-player
//! ```
//!
//! Prints every command it receives ("PlayPause → Playing"). Type `play` or
//! `pause` and Enter to change its status from outside, `quit` to leave.

#![allow(clippy::print_stdout, clippy::print_stderr)]

#[cfg(target_os = "linux")]
mod player {
    use std::io::{BufRead, Write};
    use std::sync::{Arc, Mutex};

    pub struct Player {
        pub status: Arc<Mutex<&'static str>>,
    }

    impl Player {
        fn log(&self, method: &str) {
            let status = *self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            println!("{method} → {status}");
            let _ = std::io::stdout().flush();
        }

        fn set(&self, status: &'static str) {
            *self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
        }
    }

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        fn play_pause(&self) {
            let playing = *self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                == "Playing";
            self.set(if playing { "Paused" } else { "Playing" });
            self.log("PlayPause");
        }
        fn play(&self) {
            self.set("Playing");
            self.log("Play");
        }
        fn pause(&self) {
            self.set("Paused");
            self.log("Pause");
        }
        fn stop(&self) {
            self.set("Stopped");
            self.log("Stop");
        }
        fn next(&self) {
            self.log("Next");
        }
        fn previous(&self) {
            self.log("Previous");
        }
        #[zbus(property)]
        fn playback_status(&self) -> String {
            (*self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner))
            .to_owned()
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let status = Arc::new(Mutex::new("Paused"));
        let _conn = zbus::blocking::connection::Builder::session()?
            .name("org.mpris.MediaPlayer2.padboard_test")?
            .serve_at(
                "/org/mpris/MediaPlayer2",
                Player {
                    status: Arc::clone(&status),
                },
            )?
            .build()?;
        println!("Fake player ready (Paused)");
        let _ = std::io::stdout().flush();
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            let next = match line.trim() {
                "play" => "Playing",
                "pause" => "Paused",
                "quit" => break,
                _ => continue,
            };
            *status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
            println!("status → {next}");
            let _ = std::io::stdout().flush();
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    player::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Linux only");
}
