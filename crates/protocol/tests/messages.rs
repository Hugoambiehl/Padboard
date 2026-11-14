use padboard_protocol::{
    AuthStatus, ButtonEvent, ButtonView, Category, ErrorCode, ErrorPayload, Folder, GridSize,
    Heartbeat, HelloPayload, Layout, Message, NavigatePayload, NavigateTarget, PROTOCOL_VERSION,
    Page, PairFailurePayload, PairFailureReason, PairRequestPayload, PairSuccessPayload,
    ProfileInfo, REDACTED, SessionMode, SystemStatusPayload, TabletCommandPayload, TabletOrder,
    TabletSettingsPayload, TabletStatusPayload, UPDATE_CHUNK_SIZE, UpdateOfferPayload,
    UpdateOutcome, UpdateRequestPayload, UpdateResultPayload, WelcomePayload, is_compatible,
    parse_update_chunk, short_fingerprint, speaks_core, update_chunk_frame,
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
        icon: None,
        active: false,
        unavailable: false,
        badge: None,
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
        folders: Vec::new(),
    }
}

fn all_messages() -> Vec<Message> {
    vec![
        Message::Hello(HelloPayload {
            protocol_version: PROTOCOL_VERSION,
            device_name: "Tab".into(),
            device_id: "dev-1".into(),
            token: None,
            app_version: None,
            app_version_code: None,
            silent_install: None,
        }),
        Message::Hello(HelloPayload {
            protocol_version: PROTOCOL_VERSION,
            device_name: "Tab".into(),
            device_id: "dev-1".into(),
            token: Some("secret".into()),
            app_version: Some("0.2.0".into()),
            app_version_code: Some(20_099),
            silent_install: Some(true),
        }),
        Message::Welcome(WelcomePayload {
            protocol_version: PROTOCOL_VERSION,
            server_name: "PC".into(),
            server_id: "pc-1".into(),
            auth: AuthStatus::PairingRequired,
            mode: None,
        }),
        Message::Welcome(WelcomePayload {
            protocol_version: PROTOCOL_VERSION,
            server_name: "PC".into(),
            server_id: "pc-1".into(),
            auth: AuthStatus::Authenticated,
            mode: Some(SessionMode::UpdateOnly),
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
        Message::PairRequest(PairRequestPayload {
            pin: "012345".into(),
        }),
        Message::PairSuccess(PairSuccessPayload {
            token: "secret".into(),
        }),
        Message::PairFailure(PairFailurePayload {
            reason: PairFailureReason::Locked,
            attempts_left: 0,
        }),
        Message::ButtonUpdate(ButtonView {
            active: true,
            badge: Some("REC".into()),
            ..button("a", 0, 0)
        }),
        Message::Navigate(NavigatePayload {
            to: NavigateTarget::Folder,
            id: Some("f".into()),
        }),
        Message::Navigate(NavigatePayload {
            to: NavigateTarget::Back,
            id: None,
        }),
        Message::TabletStatus(TabletStatusPayload { kiosk: false }),
        Message::TabletCommand(TabletCommandPayload {
            order: TabletOrder::ExitKiosk,
        }),
        Message::TabletSettings(TabletSettingsPayload {
            screensaver: false,
            screensaver_delay_seconds: 60,
        }),
        Message::SystemStatus(SystemStatusPayload {
            microphone_muted: false,
            recording: true,
            streaming: false,
        }),
        Message::UpdateOffer(UpdateOfferPayload {
            version: "0.2.0".into(),
            version_code: 20_099,
            size: 12_345_678,
            sha256: "ab".repeat(32),
        }),
        Message::UpdateRequest(UpdateRequestPayload {
            offset: UPDATE_CHUNK_SIZE,
        }),
        Message::UpdateResult(UpdateResultPayload {
            outcome: UpdateOutcome::BadCertificate,
        }),
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
fn optional_fields_are_omitted_when_absent() {
    let value = serde_json::to_value(button("a", 0, 0)).unwrap();
    for field in ["subtitle", "icon", "badge"] {
        assert!(value.get(field).is_none(), "{field}");
    }
    // The state flags are always sent (required on the Kotlin side).
    assert_eq!(value["active"], json!(false));
    assert_eq!(value["unavailable"], json!(false));
    let parsed: ButtonView = serde_json::from_value(json!({
        "id": "a", "row": 0, "col": 0, "label": "a", "category": "grey",
        "active": false, "unavailable": false
    }))
    .unwrap();
    assert_eq!(parsed.subtitle, None);
    assert_eq!(parsed.badge, None);
}

#[test]
fn version_compatibility() {
    assert!(is_compatible(PROTOCOL_VERSION));
    assert!(!is_compatible(PROTOCOL_VERSION + 1));
    assert!(!is_compatible(0));
    // The frozen core: from protocol 5 on, whatever comes later.
    assert!(!speaks_core(4));
    assert!(speaks_core(5));
    assert!(speaks_core(PROTOCOL_VERSION + 10));
}

#[test]
fn update_chunk_frames() {
    let frame = update_chunk_frame(0x0102_0304, b"apk");
    assert_eq!(frame, [1, 2, 3, 4, b'a', b'p', b'k']);
    assert_eq!(parse_update_chunk(&frame), Some((0x0102_0304, &b"apk"[..])));
    assert_eq!(parse_update_chunk(&[0, 0, 0, 7]), Some((7, &b""[..])));
    assert_eq!(parse_update_chunk(&[0, 0, 7]), None);
}

/// A later protocol's `hello` / `welcome` (fields added) still decode: the
/// frozen core only grows optional fields, and unknown ones are ignored.
#[test]
fn a_later_core_still_decodes() {
    let hello = Message::from_json(
        r#"{"type":"hello","payload":{"protocolVersion":9,"deviceName":"Tab","deviceId":"d",
            "token":"t","appVersion":"3.0.0","appVersionCode":3000099,"silentInstall":true,
            "somethingNew":{"a":1}}}"#,
    )
    .unwrap();
    let Message::Hello(hello) = hello else {
        panic!("hello")
    };
    assert_eq!(hello.protocol_version, 9);
    assert_eq!(hello.app_version_code, Some(3_000_099));

    let welcome = Message::from_json(
        r#"{"type":"welcome","payload":{"protocolVersion":9,"serverName":"PC","serverId":"s",
            "auth":"authenticated","mode":"update_only","later":true}}"#,
    )
    .unwrap();
    assert!(matches!(
        welcome,
        Message::Welcome(WelcomePayload {
            mode: Some(SessionMode::UpdateOnly),
            ..
        })
    ));
    let offer = Message::from_json(
        r#"{"type":"update:offer","payload":{"version":"9.0.0","versionCode":9000099,
            "size":1,"sha256":"00","signedBy":"later"}}"#,
    )
    .unwrap();
    assert!(matches!(offer, Message::UpdateOffer(_)));
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

#[test]
fn hello_token_is_omitted_before_pairing() {
    let value = serde_json::to_value(HelloPayload {
        protocol_version: PROTOCOL_VERSION,
        device_name: "Tab".into(),
        device_id: "dev-1".into(),
        token: None,
        app_version: None,
        app_version_code: None,
        silent_install: None,
    })
    .unwrap();
    assert!(value.get("token").is_none());
}

#[test]
fn enum_values_are_snake_case() {
    assert_eq!(json!(AuthStatus::TokenRejected), json!("token_rejected"));
    assert_eq!(
        json!(PairFailureReason::NoActivePin),
        json!("no_active_pin")
    );
    assert_eq!(json!(ErrorCode::Superseded), json!("superseded"));
}

#[test]
fn short_fingerprint_format() {
    for spki in [&b""[..], b"padboard", &[0xff; 91]] {
        let code = short_fingerprint(spki);
        assert_eq!(code.len(), 14, "{code}");
        let groups: Vec<_> = code.split('-').collect();
        assert_eq!(groups.len(), 3, "{code}");
        assert!(groups.iter().all(|g| g.len() == 4), "{code}");
        // Crockford base32: no ambiguous I, L, O, U.
        assert!(
            code.chars()
                .all(|c| c == '-' || c.is_ascii_digit() || "ABCDEFGHJKMNPQRSTVWXYZ".contains(c)),
            "{code}"
        );
    }
    assert_ne!(short_fingerprint(b"a"), short_fingerprint(b"b"));
}

#[test]
fn folders_share_the_id_and_cell_rules() {
    let mut l = layout(2, 2, vec![vec![button("a", 0, 0)]]);
    l.folders.push(Folder {
        id: "f".into(),
        name: "F".into(),
        buttons: vec![button("b", 1, 1)],
    });
    assert!(l.validate().is_ok());

    let mut dup = l.clone();
    dup.folders[0].buttons.push(button("a", 0, 0));
    assert!(dup.validate().unwrap_err().contains("duplicate button id"));

    let mut outside = l.clone();
    outside.folders[0].buttons[0].col = 2;
    assert!(outside.validate().unwrap_err().contains("outside"));

    let mut same_id = l;
    same_id.folders[0].id = "page-0".into();
    assert!(
        same_id
            .validate()
            .unwrap_err()
            .contains("duplicate page or folder id")
    );
}

#[test]
fn debug_output_never_shows_a_secret() {
    let messages = [
        Message::Hello(HelloPayload {
            protocol_version: PROTOCOL_VERSION,
            device_name: "Tab".into(),
            device_id: "dev-1".into(),
            token: Some("tok-SECRET-1".into()),
            app_version: None,
            app_version_code: None,
            silent_install: None,
        }),
        Message::PairRequest(PairRequestPayload {
            pin: "SECRET-PIN".into(),
        }),
        Message::PairSuccess(PairSuccessPayload {
            token: "tok-SECRET-2".into(),
        }),
    ];
    for message in messages {
        for text in [format!("{message:?}"), format!("{message:#?}")] {
            assert!(!text.contains("SECRET"), "{text}");
            assert!(text.contains(REDACTED), "{text}");
        }
    }
    // The other fields are still shown.
    let hello = format!(
        "{:?}",
        HelloPayload {
            protocol_version: 1,
            device_name: "Tab".into(),
            device_id: "dev-1".into(),
            token: None,
            app_version: None,
            app_version_code: None,
            silent_install: None,
        }
    );
    assert!(
        hello.contains("dev-1") && hello.contains("token: None"),
        "{hello}"
    );
}
