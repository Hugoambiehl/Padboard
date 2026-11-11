//! JSON shape and validation of every kind of action.

#![allow(clippy::unwrap_used)]

use padboard_actions::{
    Action, ActionError, Hotkey, Key, MAX_MULTI_STEPS, MediaAction, MediaCommand, MicrophoneAction,
    MicrophoneCommand, MultiAction, MultiStep, NavigateAction, NavigateTo, ObsAction, ObsCommand,
    OpenAction, VolumeAction, VolumeCommand,
};
use serde_json::json;

fn round_trip(action: &Action, expected: serde_json::Value) {
    assert_eq!(serde_json::to_value(action).unwrap(), expected);
    let back: Action = serde_json::from_value(expected).unwrap();
    assert_eq!(&back, action);
}

fn invalid(action: &Action) -> String {
    match action.validate() {
        Err(ActionError::Invalid(m)) => m,
        other => panic!("{action:?} should be invalid, got {other:?}"),
    }
}

#[test]
fn every_kind_has_a_stable_json_shape() {
    round_trip(
        &Action::Hotkey(Hotkey::new([Key::ControlLeft, Key::KeyC])),
        json!({"type": "hotkey", "params": {"keys": ["ControlLeft", "KeyC"]}}),
    );
    round_trip(
        &Action::Media(MediaAction {
            command: MediaCommand::PlayPause,
        }),
        json!({"type": "media", "params": {"command": "play_pause"}}),
    );
    round_trip(
        &Action::Volume(VolumeAction {
            command: VolumeCommand::Set,
            value: Some(40),
        }),
        json!({"type": "volume", "params": {"command": "set", "value": 40}}),
    );
    round_trip(
        &Action::Microphone(MicrophoneAction {
            command: MicrophoneCommand::ToggleMute,
        }),
        json!({"type": "microphone", "params": {"command": "toggle_mute"}}),
    );
    round_trip(
        &Action::Open(OpenAction {
            target: "https://example.org".into(),
        }),
        json!({"type": "open", "params": {"target": "https://example.org"}}),
    );
    round_trip(&Action::Lock, json!({"type": "lock"}));
    round_trip(
        &Action::Navigate(NavigateAction {
            to: NavigateTo::Folder,
            id: Some("f1".into()),
        }),
        json!({"type": "navigate", "params": {"to": "folder", "id": "f1"}}),
    );
    round_trip(
        &Action::Multi(MultiAction {
            steps: vec![MultiStep {
                action: Action::Lock,
                delay_ms: 200,
            }],
        }),
        json!({"type": "multi", "params": {"steps": [{"action": {"type": "lock"}, "delayMs": 200}]}}),
    );
    round_trip(
        &Action::Obs(ObsAction {
            command: ObsCommand::SetScene,
            target: Some("Jeu".into()),
        }),
        json!({"type": "obs", "params": {"command": "set_scene", "target": "Jeu"}}),
    );
}

#[test]
fn system_kinds_are_the_ones_executors_run() {
    assert!(Action::Lock.is_system());
    assert!(
        Action::Media(MediaAction {
            command: MediaCommand::Next
        })
        .is_system()
    );
    assert!(
        !Action::Navigate(NavigateAction {
            to: NavigateTo::Next,
            id: None
        })
        .is_system()
    );
    assert!(
        !Action::Obs(ObsAction {
            command: ObsCommand::ToggleRecord,
            target: None
        })
        .is_system()
    );
}

#[test]
fn volume_values_are_checked() {
    let volume = |command, value| Action::Volume(VolumeAction { command, value });
    assert!(volume(VolumeCommand::Up, None).validate().is_ok());
    assert!(volume(VolumeCommand::Down, Some(10)).validate().is_ok());
    assert!(volume(VolumeCommand::ToggleMute, None).validate().is_ok());
    assert!(volume(VolumeCommand::Set, Some(0)).validate().is_ok());
    invalid(&volume(VolumeCommand::Up, Some(0)));
    invalid(&volume(VolumeCommand::Set, None));
    invalid(&volume(VolumeCommand::Set, Some(101)));
}

#[test]
fn targets_and_names_are_required() {
    invalid(&Action::Open(OpenAction {
        target: "  ".into(),
    }));
    invalid(&Action::Open(OpenAction {
        target: "a\0b".into(),
    }));
    invalid(&Action::Navigate(NavigateAction {
        to: NavigateTo::Page,
        id: None,
    }));
    assert!(
        Action::Navigate(NavigateAction {
            to: NavigateTo::Back,
            id: None
        })
        .validate()
        .is_ok()
    );
    invalid(&Action::Obs(ObsAction {
        command: ObsCommand::ToggleInputMute,
        target: None,
    }));
    assert!(
        Action::Obs(ObsAction {
            command: ObsCommand::ToggleStream,
            target: None
        })
        .validate()
        .is_ok()
    );
}

#[test]
fn multi_actions_are_bounded_and_flat() {
    let step = |action| MultiStep {
        action,
        delay_ms: 0,
    };
    invalid(&Action::Multi(MultiAction { steps: vec![] }));
    invalid(&Action::Multi(MultiAction {
        steps: vec![step(Action::Lock); MAX_MULTI_STEPS + 1],
    }));
    invalid(&Action::Multi(MultiAction {
        steps: vec![step(Action::Multi(MultiAction {
            steps: vec![step(Action::Lock)],
        }))],
    }));
    let message = invalid(&Action::Multi(MultiAction {
        steps: vec![step(Action::Lock), step(Action::Hotkey(Hotkey::new([])))],
    }));
    assert!(message.starts_with("step 2"), "{message}");
    invalid(&Action::Multi(MultiAction {
        steps: vec![MultiStep {
            action: Action::Lock,
            delay_ms: 60_000,
        }],
    }));
}

#[test]
fn drafts_accept_missing_values_but_not_out_of_bounds_ones() {
    let empty_hotkey = Action::Hotkey(Hotkey::new([]));
    assert!(empty_hotkey.validate_draft().is_ok());
    invalid(&empty_hotkey);
    let open = Action::Open(OpenAction {
        target: String::new(),
    });
    assert!(open.validate_draft().is_ok());
    invalid(&open);
    let scene = Action::Obs(ObsAction {
        command: ObsCommand::SetScene,
        target: None,
    });
    assert!(scene.validate_draft().is_ok());
    let multi = Action::Multi(MultiAction {
        steps: vec![MultiStep {
            action: empty_hotkey,
            delay_ms: 0,
        }],
    });
    assert!(multi.validate_draft().is_ok());
    invalid(&multi);
    // Bounds hold in drafts too.
    let blank = Action::Open(OpenAction {
        target: "   ".into(),
    });
    assert!(blank.validate_draft().is_err());
    let loud = Action::Volume(VolumeAction {
        command: VolumeCommand::Set,
        value: Some(150),
    });
    assert!(loud.validate_draft().is_err());
}
