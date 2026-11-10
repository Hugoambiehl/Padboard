//! UI state, built from [`ServerEvent`]s and sent whole to the frontend on
//! every change (it is small). Mirrored by hand in `src/types/status.ts`.
//!
//! Only stable codes and raw values go to the frontend; every human-readable
//! text is produced there through vue-i18n.

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use padboard_protocol::Layout;
use serde::Serialize;

use crate::adb::UsbStatus;
use crate::auth::{AuthChange, PairedDeviceView, PairingView, Transport};
use crate::netif::LanAddress;
use crate::server::{ActionOutcome, ServerEvent};
use crate::settings::Settings;
use crate::wifi::MdnsStatus;

/// Number of entries kept in the activity log.
const MAX_ACTIVITY: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub server: ServerStatus,
    pub clients: Vec<ClientInfo>,
    pub pairing: PairingView,
    pub paired_devices: Vec<PairedDeviceView>,
    /// `adb` and the devices plugged over USB.
    pub usb: UsbStatus,
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
    /// Name announced to tablets (hostname).
    pub name: String,
    pub usb: ListenerStatus,
    pub wifi: WifiStatus,
    /// Environment values that were ignored.
    pub invalid_settings: Vec<InvalidSetting>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    pub state: ListenerState,
    pub port: u16,
    /// Address the listener tries to bind / is bound to, e.g. `127.0.0.1:47321`.
    pub address: String,
    /// OS error when binding failed (English, shown as a detail).
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListenerState {
    Off,
    Starting,
    Listening,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiStatus {
    /// Effective state: the UI setting, unless `PADBOARD_BIND` forces it.
    pub enabled: bool,
    /// `Some` when `PADBOARD_BIND` forces Wi-Fi on or off (the toggle is locked).
    pub forced: Option<bool>,
    pub listener: ListenerStatus,
    /// IPv4 addresses of this PC, physical interfaces (Wi-Fi, Ethernet) first.
    pub lan_addresses: Vec<LanAddress>,
    /// Short fingerprint of the TLS key, shown when pairing.
    pub fingerprint: Option<String>,
    /// Why the TLS identity could not be created (Wi-Fi unavailable).
    pub tls_error: Option<String>,
    pub mdns: MdnsStatus,
}

/// Initial values of [`Snapshot::new`].
#[derive(Debug, Clone)]
pub struct Init<'a> {
    pub settings: &'a Settings,
    pub server_name: String,
    pub lan_addresses: Vec<LanAddress>,
    pub layout: &'a Layout,
    pub wifi_enabled: bool,
    pub fingerprint: Option<String>,
    pub tls_error: Option<String>,
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
    pub device_id: String,
    pub device_name: String,
    pub transport: Transport,
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
    pub fn new(init: Init<'_>) -> Self {
        let Init {
            settings,
            server_name,
            lan_addresses,
            layout,
            wifi_enabled,
            fingerprint,
            tls_error,
        } = init;
        let labels = layout
            .pages
            .iter()
            .flat_map(|p| &p.buttons)
            .map(|b| (b.id.clone(), b.label.clone()))
            .collect();
        let wifi_starting = wifi_enabled && tls_error.is_none();
        Self {
            server: ServerStatus {
                name: server_name,
                usb: ListenerStatus {
                    state: ListenerState::Starting,
                    port: settings.usb_port,
                    address: settings.usb_addr().to_string(),
                    error_detail: None,
                },
                wifi: WifiStatus {
                    enabled: wifi_enabled,
                    forced: settings.wifi_forced,
                    listener: ListenerStatus {
                        state: if wifi_starting {
                            ListenerState::Starting
                        } else {
                            ListenerState::Off
                        },
                        port: settings.wifi_port,
                        address: settings.wifi_addr().to_string(),
                        error_detail: None,
                    },
                    lan_addresses,
                    fingerprint,
                    tls_error,
                    mdns: MdnsStatus::Off,
                },
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
            pairing: PairingView::Idle,
            paired_devices: Vec::new(),
            usb: UsbStatus::default(),
            activity: VecDeque::new(),
            labels,
            next_activity_id: 1,
        }
    }

    fn listener(&mut self, transport: Transport) -> &mut ListenerStatus {
        match transport {
            Transport::Usb => &mut self.server.usb,
            Transport::Wifi => &mut self.server.wifi.listener,
        }
    }

    pub fn apply(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Listening { transport, addr } => {
                let listener = self.listener(transport);
                listener.state = ListenerState::Listening;
                listener.address = addr.to_string();
                listener.error_detail = None;
            }
            ServerEvent::BindFailed {
                transport,
                addr,
                error,
            } => {
                let listener = self.listener(transport);
                listener.state = ListenerState::Error;
                listener.address = addr.to_string();
                listener.error_detail = Some(error);
            }
            ServerEvent::Stopped { transport } => {
                let listener = self.listener(transport);
                listener.state = ListenerState::Off;
                listener.error_detail = None;
                // Connections of that transport were closed with the listener.
                self.clients.retain(|c| c.transport != transport);
            }
            ServerEvent::WifiEnabled(enabled) => {
                self.server.wifi.enabled = enabled;
                let wifi = &mut self.server.wifi;
                if enabled && wifi.listener.state == ListenerState::Off && wifi.tls_error.is_none()
                {
                    wifi.listener.state = ListenerState::Starting;
                }
            }
            ServerEvent::Mdns(status) => self.server.wifi.mdns = status,
            ServerEvent::Usb(status) => self.usb = status,
            ServerEvent::LanAddresses(addresses) => self.server.wifi.lan_addresses = addresses,
            ServerEvent::ClientConnected {
                id,
                device_id,
                device_name,
                peer,
                transport,
            } => self.clients.push(ClientInfo {
                id,
                device_id,
                device_name,
                transport,
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
            ServerEvent::Auth(AuthChange::Pairing(view)) => self.pairing = view,
            ServerEvent::Auth(AuthChange::PairedDevices(devices)) => {
                self.paired_devices = devices;
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
        let settings = Settings::from_vars(Some("bogus"), None, None);
        let layout = crate::demo::profile().layout;
        Snapshot::new(Init {
            settings: &settings,
            server_name: "PC".into(),
            lan_addresses: vec![LanAddress {
                address: "192.168.1.20".into(),
                interface: "wlo1".into(),
                physical: true,
            }],
            layout: &layout,
            wifi_enabled: false,
            fingerprint: Some("AAAA-BBBB-CCCC".into()),
            tls_error: None,
        })
    }

    fn peer() -> SocketAddr {
        "127.0.0.1:50000".parse().unwrap()
    }

    #[test]
    fn starts_in_starting_state_and_reports_invalid_settings() {
        let s = snapshot();
        assert_eq!(s.server.usb.state, ListenerState::Starting);
        assert_eq!(s.server.wifi.listener.state, ListenerState::Off);
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
            transport: Transport::Usb,
            addr,
            error: "address in use".into(),
        });
        assert_eq!(s.server.usb.state, ListenerState::Error);
        assert_eq!(s.server.usb.error_detail.as_deref(), Some("address in use"));
        s.apply(ServerEvent::Listening {
            transport: Transport::Usb,
            addr,
        });
        assert_eq!(s.server.usb.state, ListenerState::Listening);
        assert_eq!(s.server.usb.error_detail, None);
    }

    #[test]
    fn tracks_clients_and_activity() {
        let mut s = snapshot();
        s.apply(ServerEvent::ClientConnected {
            id: 1,
            device_id: "tab".into(),
            device_name: "Tab".into(),
            peer: peer(),
            transport: Transport::Usb,
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
    fn tracks_wifi_toggle_and_its_clients() {
        let mut s = snapshot();
        s.apply(ServerEvent::WifiEnabled(true));
        assert!(s.server.wifi.enabled);
        assert_eq!(s.server.wifi.listener.state, ListenerState::Starting);
        s.apply(ServerEvent::Listening {
            transport: Transport::Wifi,
            addr: "0.0.0.0:47322".parse().unwrap(),
        });
        s.apply(ServerEvent::ClientConnected {
            id: 1,
            device_id: "a".into(),
            device_name: "A".into(),
            peer: peer(),
            transport: Transport::Wifi,
        });
        s.apply(ServerEvent::ClientConnected {
            id: 2,
            device_id: "b".into(),
            device_name: "B".into(),
            peer: peer(),
            transport: Transport::Usb,
        });
        s.apply(ServerEvent::Mdns(MdnsStatus::Announced));
        assert_eq!(s.server.wifi.mdns, MdnsStatus::Announced);

        s.apply(ServerEvent::WifiEnabled(false));
        s.apply(ServerEvent::Stopped {
            transport: Transport::Wifi,
        });
        assert_eq!(s.server.wifi.listener.state, ListenerState::Off);
        assert_eq!(s.clients.len(), 1);
        assert_eq!(s.clients[0].transport, Transport::Usb);
    }

    #[test]
    fn tracks_pairing_and_paired_devices() {
        let mut s = snapshot();
        let view = PairingView::Active {
            pin: "123456".into(),
            expires_at_ms: 1,
            attempts_left: 5,
        };
        s.apply(ServerEvent::Auth(AuthChange::Pairing(view.clone())));
        assert_eq!(s.pairing, view);
        s.apply(ServerEvent::Auth(AuthChange::PairedDevices(vec![
            PairedDeviceView {
                device_id: "tab".into(),
                name: "Tab".into(),
                paired_at_ms: 1000,
                last_seen_ms: None,
            },
        ])));
        assert_eq!(s.paired_devices.len(), 1);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["pairing"]["state"], "active");
        assert_eq!(v["pairing"]["expiresAtMs"], 1);
        assert_eq!(v["pairedDevices"][0]["deviceId"], "tab");
    }

    #[test]
    fn serializes_in_camel_case() {
        let v = serde_json::to_value(snapshot()).unwrap();
        assert_eq!(v["server"]["usb"]["state"], "starting");
        assert_eq!(v["server"]["wifi"]["listener"]["state"], "off");
        assert_eq!(v["server"]["wifi"]["fingerprint"], "AAAA-BBBB-CCCC");
        assert_eq!(v["server"]["wifi"]["mdns"]["state"], "off");
        assert!(v["server"]["wifi"]["lanAddresses"].is_array());
        assert_eq!(v["usb"]["adb"]["state"], "searching");
        assert!(v.get("labels").is_none());
    }
}
