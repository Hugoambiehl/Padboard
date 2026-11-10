//! UI state, built from [`ServerEvent`]s and sent whole to the frontend on
//! every change (it is small). Mirrored by hand in `src/types/status.ts`.
//!
//! Only stable codes and raw values go to the frontend; every human-readable
//! text is produced there through vue-i18n.

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use padboard_protocol::Layout;
use serde::Serialize;

use crate::server::{ActionOutcome, ServerEvent};
use crate::settings::{BindMode, Settings};

/// Number of entries kept in the activity log.
const MAX_ACTIVITY: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub server: ServerStatus,
    pub clients: Vec<ClientInfo>,
    /// Most recent first.
    pub activity: VecDeque<ActivityEntry>,
    #[serde(skip)]
    labels: HashMap<String, String>,
    #[serde(skip)]
    next_activity_id: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub state: ServerState,
    pub bind_mode: BindMode,
    pub port: u16,
    /// Address the server tries to bind / is bound to, e.g. `127.0.0.1:47321`.
    pub address: String,
    /// IPv4 addresses of this PC on the local network (what to type on the tablet).
    pub lan_addresses: Vec<String>,
    /// OS error when binding failed (English, shown as a detail).
    pub error_detail: Option<String>,
    /// Environment values that were ignored.
    pub invalid_settings: Vec<InvalidSetting>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerState {
    Starting,
    Listening,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidSetting {
    pub variable: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub id: u64,
    pub device_name: String,
    pub peer: String,
    pub connected_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: u64,
    pub at_ms: u64,
    pub device_name: Option<String>,
    pub button_id: String,
    /// Button label at the time of the press, if the button exists.
    pub label: Option<String>,
    pub outcome: ActionOutcome,
}

impl Snapshot {
    #[must_use]
    pub fn new(settings: &Settings, lan_addresses: Vec<String>, layout: &Layout) -> Self {
        let labels = layout
            .pages
            .iter()
            .flat_map(|p| &p.buttons)
            .map(|b| (b.id.clone(), b.label.clone()))
            .collect();
        Self {
            server: ServerStatus {
                state: ServerState::Starting,
                bind_mode: settings.bind,
                port: settings.port,
                address: settings.socket_addr().to_string(),
                lan_addresses,
                error_detail: None,
                invalid_settings: settings
                    .invalid
                    .iter()
                    .map(|(variable, value)| InvalidSetting {
                        variable: (*variable).to_owned(),
                        value: value.clone(),
                    })
                    .collect(),
            },
            clients: Vec::new(),
            activity: VecDeque::new(),
            labels,
            next_activity_id: 1,
        }
    }

    pub fn apply(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Listening(addr) => {
                self.server.state = ServerState::Listening;
                self.server.address = addr.to_string();
                self.server.error_detail = None;
            }
            ServerEvent::BindFailed { addr, error } => {
                self.server.state = ServerState::Error;
                self.server.address = addr.to_string();
                self.server.error_detail = Some(error);
            }
            ServerEvent::ClientConnected {
                id,
                device_name,
                peer,
            } => self.clients.push(ClientInfo {
                id,
                device_name,
                peer: peer.to_string(),
                connected_at_ms: now_ms(),
            }),
            ServerEvent::ClientDisconnected { id } => self.clients.retain(|c| c.id != id),
            ServerEvent::ButtonPressed {
                client_id,
                button_id,
                outcome,
            } => {
                let entry = ActivityEntry {
                    id: self.next_activity_id,
                    at_ms: now_ms(),
                    device_name: self
                        .clients
                        .iter()
                        .find(|c| c.id == client_id)
                        .map(|c| c.device_name.clone()),
                    label: self.labels.get(&button_id).cloned(),
                    button_id,
                    outcome,
                };
                self.next_activity_id += 1;
                self.activity.push_front(entry);
                self.activity.truncate(MAX_ACTIVITY);
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    fn snapshot() -> Snapshot {
        let settings = Settings::from_vars(Some("bogus"), None);
        Snapshot::new(
            &settings,
            vec!["192.168.1.20".into()],
            &crate::demo::profile().layout,
        )
    }

    fn peer() -> SocketAddr {
        "127.0.0.1:50000".parse().unwrap()
    }

    #[test]
    fn starts_in_starting_state_and_reports_invalid_settings() {
        let s = snapshot();
        assert_eq!(s.server.state, ServerState::Starting);
        assert_eq!(s.server.bind_mode, BindMode::Loopback);
        assert_eq!(
            s.server.invalid_settings,
            [InvalidSetting {
                variable: "PADBOARD_BIND".into(),
                value: "bogus".into()
            }]
        );
    }

    #[test]
    fn tracks_server_state() {
        let mut s = snapshot();
        let addr: SocketAddr = "127.0.0.1:47321".parse().unwrap();
        s.apply(ServerEvent::BindFailed {
            addr,
            error: "address in use".into(),
        });
        assert_eq!(s.server.state, ServerState::Error);
        assert_eq!(s.server.error_detail.as_deref(), Some("address in use"));
        s.apply(ServerEvent::Listening(addr));
        assert_eq!(s.server.state, ServerState::Listening);
        assert_eq!(s.server.error_detail, None);
    }

    #[test]
    fn tracks_clients_and_activity() {
        let mut s = snapshot();
        s.apply(ServerEvent::ClientConnected {
            id: 1,
            device_name: "Tab".into(),
            peer: peer(),
        });
        assert_eq!(s.clients.len(), 1);
        s.apply(ServerEvent::ButtonPressed {
            client_id: 1,
            button_id: "terminal".into(),
            outcome: ActionOutcome::Ok,
        });
        let entry = &s.activity[0];
        assert_eq!(entry.label.as_deref(), Some("Terminal"));
        assert_eq!(entry.device_name.as_deref(), Some("Tab"));
        s.apply(ServerEvent::ClientDisconnected { id: 1 });
        assert!(s.clients.is_empty());
    }

    #[test]
    fn activity_is_bounded_and_most_recent_first() {
        let mut s = snapshot();
        for i in 0..(MAX_ACTIVITY + 10) {
            s.apply(ServerEvent::ButtonPressed {
                client_id: 9,
                button_id: format!("b{i}"),
                outcome: ActionOutcome::Ok,
            });
        }
        assert_eq!(s.activity.len(), MAX_ACTIVITY);
        assert_eq!(s.activity[0].button_id, format!("b{}", MAX_ACTIVITY + 9));
        assert_eq!(s.activity[0].label, None);
    }

    #[test]
    fn serializes_in_camel_case() {
        let v = serde_json::to_value(snapshot()).unwrap();
        assert_eq!(v["server"]["bindMode"], "loopback");
        assert_eq!(v["server"]["state"], "starting");
        assert!(v["server"]["lanAddresses"].is_array());
        assert!(v.get("labels").is_none());
    }
}
