//! Button states: what the PC observes (microphone, volume, media, OBS) turned
//! into what each button looks like on the tablet.
//!
//! The layout built from the configuration ([`crate::config::ActiveLayout`])
//! is the "resting" appearance; [`resolve`] applies the observed
//! [`SystemState`] on top of it. Sessions send the resolved layout when a
//! tablet connects (so it gets every state at once), then, on each state
//! change, a `button:update` for the buttons whose appearance [`changed`]
//! only: the sound server sends several identical notifications for one
//! change, and a state that does not show on any button sends nothing.

use std::collections::HashMap;

use padboard_actions::state::AudioState;
use padboard_actions::{Action, MediaCommand, ObsCommand, VolumeCommand};
use padboard_protocol::{ButtonIcon, ButtonView, Category, Layout};

/// Something observed, or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Observed<T> {
    /// Not observed (yet, or in test mode): buttons keep their resting look.
    #[default]
    Unknown,
    /// Observed as missing (no sound server…): the buttons are unavailable.
    Unavailable,
    Known(T),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemState {
    pub audio: Observed<AudioState>,
    /// Whether a media player is playing (no player: `Known(false)`).
    pub media_playing: Observed<bool>,
    /// OBS, when connected (`obs.rs`); unavailable while OBS is closed.
    pub obs: Observed<crate::obs::ObsState>,
}

impl SystemState {
    /// OBS scenes and inputs, for the editor's pickers (empty while OBS is
    /// not connected).
    #[must_use]
    pub fn obs_names(&self) -> (Vec<String>, Vec<String>) {
        match &self.obs {
            Observed::Known(obs) => (obs.scenes.clone(), obs.inputs.clone()),
            _ => (Vec::new(), Vec::new()),
        }
    }
}

/// Texts shown on the tablet by the state engine (French; English comes
/// with the rest of the translations before open-sourcing).
mod text {
    pub const MUTED: &str = "Muet";
    pub const SCENE_ACTIVE: &str = "Scène active";
    pub const RECORDING: &str = "REC";
    pub const LIVE: &str = "EN DIRECT";
    pub const OUTPUT_MUTED: &str = "Coupé";
    pub fn volume(percent: u32) -> String {
        format!("Volume {percent} %")
    }
}

/// Lucide icons (ISC) used by states, converted like `src/icons/paths.ts`.
mod icons {
    pub const MIC_OFF: &[&str] = &[
        "M12 19v3",
        "M15 9.34V5a3 3 0 0 0-5.68-1.33",
        "M16.95 16.95A7 7 0 0 1 5 12v-2",
        "M18.89 13.23A7 7 0 0 0 19 12v-2",
        "m2 2 20 20",
        "M9 9v3a3 3 0 0 0 5.12 2.12",
    ];
    pub const VOLUME_X: &[&str] = &[
        "M11 4.702a.7.7 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.7.7 0 0 0 11 19.298z",
        "m16.5 14.5 5-5",
        "m16.5 9.5 5 5",
    ];
    pub const PLAY: &[&str] =
        &["M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"];
    pub const PAUSE: &[&str] = &[
        "M15 3H18A1 1 0 0 1 19 4V20A1 1 0 0 1 18 21H15A1 1 0 0 1 14 20V4A1 1 0 0 1 15 3z",
        "M6 3H9A1 1 0 0 1 10 4V20A1 1 0 0 1 9 21H6A1 1 0 0 1 5 20V4A1 1 0 0 1 6 3z",
    ];
}

fn icon(paths: &[&str]) -> Option<ButtonIcon> {
    Some(ButtonIcon {
        paths: paths.iter().map(|p| (*p).to_owned()).collect(),
    })
}

/// "Off" look: highlighted in red with its own icon and subtitle (Main.dc.html:
/// "Micro coupé · Muet").
fn switched_off(button: &mut ButtonView, paths: &[&str], subtitle: &str) {
    button.active = true;
    button.category = Category::Red;
    button.icon = icon(paths);
    button.subtitle = Some(subtitle.to_owned());
}

/// The appearance of one button for `state`.
fn apply(button: &mut ButtonView, action: &Action, state: &SystemState) {
    match (action, state.audio) {
        (Action::Microphone(_), Observed::Unavailable) => button.unavailable = true,
        (Action::Microphone(_), Observed::Known(audio)) => match audio.input_muted {
            None => button.unavailable = true,
            Some(true) => switched_off(button, icons::MIC_OFF, text::MUTED),
            Some(false) => {}
        },
        (Action::Volume(_), Observed::Unavailable) => button.unavailable = true,
        (Action::Volume(v), Observed::Known(audio)) => match v.command {
            VolumeCommand::ToggleMute if audio.output_muted => {
                switched_off(button, icons::VOLUME_X, text::OUTPUT_MUTED);
            }
            VolumeCommand::ToggleMute => {}
            // The level, unless the user wrote their own subtitle.
            VolumeCommand::Up | VolumeCommand::Down | VolumeCommand::Set => {
                if button.subtitle.is_none() {
                    button.subtitle = Some(if audio.output_muted {
                        text::OUTPUT_MUTED.to_owned()
                    } else {
                        text::volume(audio.output_percent)
                    });
                }
            }
        },
        (Action::Obs(o), _) => match &state.obs {
            Observed::Unknown => {}
            Observed::Unavailable => button.unavailable = true,
            Observed::Known(obs) => match o.command {
                ObsCommand::SetScene if obs.scene.is_some() && obs.scene == o.target => {
                    button.active = true;
                    button.subtitle = Some(text::SCENE_ACTIVE.to_owned());
                }
                ObsCommand::ToggleRecord if obs.recording => {
                    button.active = true;
                    button.badge = Some(text::RECORDING.to_owned());
                }
                ObsCommand::ToggleStream if obs.streaming => {
                    button.active = true;
                    button.badge = Some(text::LIVE.to_owned());
                }
                ObsCommand::ToggleInputMute
                    if o.target
                        .as_ref()
                        .is_some_and(|t| obs.muted_inputs.contains(t)) =>
                {
                    button.active = true;
                    button.category = Category::Red;
                    button.subtitle = Some(text::OUTPUT_MUTED.to_owned());
                }
                _ => {}
            },
        },
        (Action::Media(m), _)
            if m.command == MediaCommand::PlayPause
                && state.media_playing == Observed::Known(true)
                && button.icon == icon(icons::PLAY) =>
        {
            button.icon = icon(icons::PAUSE);
        }
        _ => {}
    }
}

/// `layout` with the state of every button applied.
#[must_use]
pub fn resolve(layout: &Layout, actions: &HashMap<String, Action>, state: &SystemState) -> Layout {
    let mut resolved = layout.clone();
    let grids = resolved
        .pages
        .iter_mut()
        .map(|p| &mut p.buttons)
        .chain(resolved.folders.iter_mut().map(|f| &mut f.buttons));
    for buttons in grids {
        for button in buttons {
            if let Some(action) = actions.get(&button.id) {
                apply(button, action, state);
            }
        }
    }
    resolved
}

/// Buttons of `new` that look different in `old` (same layout, another
/// state), in layout order.
#[must_use]
pub fn changed(old: &Layout, new: &Layout) -> Vec<ButtonView> {
    let before: HashMap<&str, &ButtonView> = all(old).map(|b| (b.id.as_str(), b)).collect();
    all(new)
        .filter(|b| before.get(b.id.as_str()) != Some(b))
        .cloned()
        .collect()
}

fn all(layout: &Layout) -> impl Iterator<Item = &ButtonView> {
    layout
        .pages
        .iter()
        .flat_map(|p| &p.buttons)
        .chain(layout.folders.iter().flat_map(|f| &f.buttons))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::seed;

    fn audio(input_muted: Option<bool>, output_muted: bool, percent: u32) -> Observed<AudioState> {
        Observed::Known(AudioState {
            output_percent: percent,
            output_muted,
            input_muted,
        })
    }

    /// The demo layout, plus a microphone button.
    fn demo() -> (Layout, HashMap<String, Action>) {
        let mut active = seed::config().active_layout();
        let mut mic = active.layout.pages[0].buttons[0].clone();
        mic.id = "mic".into();
        mic.row = 2;
        mic.col = 3;
        mic.label = "Micro".into();
        mic.category = Category::Violet;
        active.layout.pages[0].buttons.push(mic);
        active.actions.insert(
            "mic".into(),
            Action::Microphone(padboard_actions::MicrophoneAction {
                command: padboard_actions::MicrophoneCommand::ToggleMute,
            }),
        );
        (active.layout, active.actions)
    }

    fn button<'a>(layout: &'a Layout, id: &str) -> &'a ButtonView {
        all(layout).find(|b| b.id == id).unwrap()
    }

    #[test]
    fn unknown_state_keeps_the_resting_look() {
        let (layout, actions) = demo();
        assert_eq!(resolve(&layout, &actions, &SystemState::default()), layout);
    }

    #[test]
    fn a_muted_microphone_shows_off() {
        let (layout, actions) = demo();
        let state = SystemState {
            audio: audio(Some(true), false, 50),
            ..SystemState::default()
        };
        let resolved = resolve(&layout, &actions, &state);
        let mic = button(&resolved, "mic");
        assert!(mic.active);
        assert_eq!(mic.category, Category::Red);
        assert_eq!(mic.subtitle.as_deref(), Some("Muet"));
        assert_eq!(mic.icon, icon(icons::MIC_OFF));
        assert_eq!(mic.label, "Micro", "the user's label stays");
        // Unmuted: the resting look.
        let state = SystemState {
            audio: audio(Some(false), false, 50),
            ..SystemState::default()
        };
        assert_eq!(
            button(&resolve(&layout, &actions, &state), "mic"),
            button(&layout, "mic")
        );
    }

    #[test]
    fn no_sound_server_or_microphone_makes_buttons_unavailable() {
        let (layout, actions) = demo();
        let state = SystemState {
            audio: Observed::Unavailable,
            ..SystemState::default()
        };
        let resolved = resolve(&layout, &actions, &state);
        assert!(button(&resolved, "mic").unavailable);
        assert!(button(&resolved, "volume-up").unavailable);
        assert!(!button(&resolved, "copy").unavailable);
        let state = SystemState {
            audio: audio(None, false, 50),
            ..SystemState::default()
        };
        let resolved = resolve(&layout, &actions, &state);
        assert!(button(&resolved, "mic").unavailable);
        assert!(!button(&resolved, "volume-up").unavailable);
    }

    #[test]
    fn volume_buttons_show_the_level_and_the_mute() {
        let (layout, actions) = demo();
        let mut l = layout.clone();
        // The demo's volume buttons have a subtitle ("Son PC"): clear one.
        l.pages[0]
            .buttons
            .iter_mut()
            .find(|b| b.id == "volume-up")
            .unwrap()
            .subtitle = None;
        let state = SystemState {
            audio: audio(Some(false), false, 64),
            ..SystemState::default()
        };
        let resolved = resolve(&l, &actions, &state);
        assert_eq!(
            button(&resolved, "volume-up").subtitle.as_deref(),
            Some("Volume 64 %")
        );
        assert_eq!(
            button(&resolved, "volume-down").subtitle.as_deref(),
            Some("Son PC")
        );
        assert!(!button(&resolved, "mute").active);
        let state = SystemState {
            audio: audio(Some(false), true, 64),
            ..SystemState::default()
        };
        let resolved = resolve(&l, &actions, &state);
        assert!(button(&resolved, "mute").active);
        assert_eq!(
            button(&resolved, "volume-up").subtitle.as_deref(),
            Some("Coupé")
        );
    }

    #[test]
    fn play_pause_shows_pause_while_playing() {
        let (layout, actions) = demo();
        let state = SystemState {
            media_playing: Observed::Known(true),
            ..SystemState::default()
        };
        let resolved = resolve(&layout, &actions, &state);
        assert_eq!(button(&resolved, "play-pause").icon, icon(icons::PAUSE));
    }

    #[test]
    fn only_buttons_whose_look_changes_are_updated() {
        let (layout, actions) = demo();
        let quiet = SystemState {
            audio: audio(Some(false), false, 40),
            ..SystemState::default()
        };
        let before = resolve(&layout, &actions, &quiet);
        // The same state notified again: nothing to send.
        assert!(changed(&before, &resolve(&layout, &actions, &quiet)).is_empty());
        // The volume moves: the demo's volume buttons have their own
        // subtitle, so nothing shows it either.
        let louder = SystemState {
            audio: audio(Some(false), false, 45),
            ..quiet.clone()
        };
        assert!(changed(&before, &resolve(&layout, &actions, &louder)).is_empty());
        // The microphone is muted: that button only.
        let muted = SystemState {
            audio: audio(Some(true), false, 45),
            ..quiet.clone()
        };
        let updates = changed(&before, &resolve(&layout, &actions, &muted));
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].id, "mic");
        assert!(updates[0].active);
    }
}
