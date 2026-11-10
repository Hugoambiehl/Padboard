use padboard_protocol::{
    ButtonEvent, ButtonView, Category, ErrorCode, ErrorPayload, GridSize, Heartbeat, HelloPayload,
    Layout, Message, PROTOCOL_VERSION, Page, ProfileInfo, WelcomePayload, is_compatible,
};
use serde_json::json;

fn button(id: &str, row: u32, col: u32) -> ButtonView {
    ButtonView {
        id: id.into(),
        row,
        col,
        label: id.into(),
        subtitle: None,
        category: Category::Grey,
    }
}

fn layout(cols: u32, rows: u32, pages: Vec<Vec<ButtonView>>) -> Layout {
    Layout {
        profile: ProfileInfo {
            id: "p".into(),
            name: "P".into(),
        },
        grid: GridSize { cols, rows },
        pages: pages
            .into_iter()
            .enumerate()
            .map(|(i, buttons)| Page {
                id: format!("page-{i}"),
                buttons,
            })
            .collect(),
    }
}

fn all_messages() -> Vec<Message> {
    vec![
        Message::Hello(HelloPayload {
            protocol_version: PROTOCOL_VERSION,
            device_name: "Tab".into(),
        }),
        Message::Welcome(WelcomePayload {
            protocol_version: PROTOCOL_VERSION,
            server_name: "PC".into(),
        }),
        Message::Error(ErrorPayload {
            code: ErrorCode::UnknownButton,
            message: "nope".into(),
        }),
        Message::LayoutSync(layout(5, 3, vec![vec![button("a", 0, 0)]])),
        Message::ButtonPress(ButtonEvent {
            button_id: "a".into(),
        }),
        Message::ButtonRelease(ButtonEvent {
            button_id: "a".into(),
        }),
        Message::Ping(Heartbeat { seq: 1 }),
        Message::Pong(Heartbeat { seq: u32::MAX }),
    ]
}

#[test]
fn every_message_round_trips() {
    for message in all_messages() {
        let json = message.to_json().unwrap();
        assert_eq!(Message::from_json(&json).unwrap(), message, "{json}");
    }
}

#[test]
fn envelope_is_type_and_payload() {
    let json = Message::ButtonPress(ButtonEvent {
        button_id: "mic".into(),
    })
    .to_json()
    .unwrap();
    assert_eq!(
        json,
        r#"{"type":"button:press","payload":{"buttonId":"mic"}}"#
    );
}

#[test]
fn type_tag_matches_serialized_type() {
    for message in all_messages() {
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], message.type_tag());
    }
}

#[test]
fn unknown_fields_are_ignored_for_forward_compatibility() {
    let json = r#"{"type":"button:press","payload":{"buttonId":"mic","pressure":0.8},"extra":1}"#;
    assert_eq!(
        Message::from_json(json).unwrap(),
        Message::ButtonPress(ButtonEvent {
            button_id: "mic".into()
        })
    );
}

#[test]
fn unknown_type_is_rejected() {
    assert!(Message::from_json(r#"{"type":"teleport","payload":{}}"#).is_err());
}

#[test]
fn missing_payload_is_rejected() {
    assert!(Message::from_json(r#"{"type":"ping"}"#).is_err());
}

#[test]
fn optional_subtitle_is_omitted_when_absent() {
    let value = serde_json::to_value(button("a", 0, 0)).unwrap();
    assert!(value.get("subtitle").is_none());
    let parsed: ButtonView = serde_json::from_value(json!({
        "id": "a", "row": 0, "col": 0, "label": "a", "category": "grey"
    }))
    .unwrap();
    assert_eq!(parsed.subtitle, None);
}

#[test]
fn version_compatibility() {
    assert!(is_compatible(PROTOCOL_VERSION));
    assert!(!is_compatible(PROTOCOL_VERSION + 1));
    assert!(!is_compatible(0));
}

#[test]
fn valid_layout_passes() {
    let l = layout(
        5,
        3,
        vec![
            vec![button("a", 0, 0), button("b", 2, 4)],
            vec![button("c", 0, 0)],
        ],
    );
    assert_eq!(l.validate(), Ok(()));
}

#[test]
fn layout_validation_rejects_bad_layouts() {
    let cases = [
        ("no page", layout(5, 3, vec![])),
        ("empty grid", layout(0, 3, vec![vec![]])),
        ("out of grid", layout(5, 3, vec![vec![button("a", 3, 0)]])),
        (
            "duplicate id across pages",
            layout(5, 3, vec![vec![button("a", 0, 0)], vec![button("a", 1, 1)]]),
        ),
        (
            "two buttons in one cell",
            layout(5, 3, vec![vec![button("a", 1, 1), button("b", 1, 1)]]),
        ),
    ];
    for (name, l) in cases {
        assert!(l.validate().is_err(), "{name} should be rejected");
    }
}
