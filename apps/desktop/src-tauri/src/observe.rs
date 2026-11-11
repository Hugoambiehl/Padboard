//! Watches the system for the button states ([`crate::appearance`]): audio
//! (change feed, see `padboard_actions::state`) and media players (polled).
//! Each watcher runs on its own thread (the sources are blocking) and only
//! publishes real changes.

use std::time::Duration;

use padboard_actions::state::{self, AudioChanges};
use tokio::sync::watch;

use crate::appearance::{Observed, SystemState};

/// Before subscribing again after the sound server went away.
const AUDIO_RETRY: Duration = Duration::from_secs(2);
/// Media players have no cheap change feed on every platform: polled.
const MEDIA_POLL: Duration = Duration::from_secs(1);

/// Starts the watchers, for the life of the app (they also run while no
/// tablet is connected, so a tablet that connects gets the current state).
pub fn start(state: &watch::Sender<SystemState>) {
    let audio = state.clone();
    spawn("padboard-audio-state", move || audio_loop(&audio));
    let media = state.clone();
    spawn("padboard-media-state", move || media_loop(&media));
}

fn spawn(name: &str, body: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(body)
    {
        tracing::error!("cannot start {name}: {e}");
    }
}

fn read_audio() -> Observed<state::AudioState> {
    match state::audio() {
        Ok(audio) => Observed::Known(audio),
        Err(e) => {
            tracing::debug!("audio state: {e}");
            Observed::Unavailable
        }
    }
}

fn publish_audio(tx: &watch::Sender<SystemState>, audio: Observed<state::AudioState>) {
    tx.send_if_modified(|s| {
        let changed = s.audio != audio;
        s.audio = audio;
        changed
    });
}

fn audio_loop(tx: &watch::Sender<SystemState>) -> ! {
    loop {
        match AudioChanges::subscribe() {
            Ok(mut changes) => {
                publish_audio(tx, read_audio());
                // Several identical notifications per change: deduplicated
                // by `publish_audio`, and again per button by the sessions.
                while changes.wait().is_ok() {
                    publish_audio(tx, read_audio());
                }
                tracing::info!("audio change feed ended, subscribing again");
            }
            Err(e) => {
                tracing::debug!("audio change feed: {e}");
                publish_audio(tx, Observed::Unavailable);
            }
        }
        std::thread::sleep(AUDIO_RETRY);
    }
}

fn media_loop(tx: &watch::Sender<SystemState>) -> ! {
    loop {
        let playing = match state::media_playing() {
            Ok(playing) => Observed::Known(playing.unwrap_or(false)),
            Err(e) => {
                tracing::debug!("media state: {e}");
                Observed::Unavailable
            }
        };
        tx.send_if_modified(|s| {
            let changed = s.media_playing != playing;
            s.media_playing = playing;
            changed
        });
        std::thread::sleep(MEDIA_POLL);
    }
}
