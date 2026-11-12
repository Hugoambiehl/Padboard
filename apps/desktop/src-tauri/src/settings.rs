//! Server settings read from the environment. They take precedence over the
//! preferences changed from the UI (`preferences.rs`).
//!
//! - `PADBOARD_BIND`: forces Wi-Fi on (`lan`, also `wifi` / `0.0.0.0`) or off
//!   (`loopback`, also `localhost` / `127.0.0.1`). Unset: the UI setting
//!   decides (off by default).
//! - `PADBOARD_PORT`: USB port (loopback only), default [`USB_PORT`].
//! - `PADBOARD_WIFI_PORT`: Wi-Fi port (all interfaces, TLS), default [`WIFI_PORT`].
//!
//! Invalid values never widen exposure: they fall back to the defaults and are
//! reported to the UI.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use padboard_protocol::{USB_PORT, WIFI_PORT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `Some` when `PADBOARD_BIND` forces Wi-Fi on or off.
    pub wifi_forced: Option<bool>,
    /// Wi-Fi is forced off by the test mode, not by `PADBOARD_BIND`.
    pub wifi_forced_by_test: bool,
    pub usb_port: u16,
    pub wifi_port: u16,
    /// Environment values that were ignored, as `(variable, value)`.
    pub invalid: Vec<(&'static str, String)>,
}

impl Settings {
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok();
        Self::from_vars(
            var("PADBOARD_BIND").as_deref(),
            var("PADBOARD_PORT").as_deref(),
            var("PADBOARD_WIFI_PORT").as_deref(),
        )
    }

    #[must_use]
    pub fn from_vars(bind: Option<&str>, usb_port: Option<&str>, wifi_port: Option<&str>) -> Self {
        let mut invalid = Vec::new();

        let wifi_forced = match bind.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") => None,
            Some("loopback" | "localhost" | "127.0.0.1") => Some(false),
            Some("lan" | "wifi" | "0.0.0.0") => Some(true),
            Some(_) => {
                invalid.push(("PADBOARD_BIND", bind.unwrap_or_default().to_owned()));
                None
            }
        };

        let mut port = |variable, value: Option<&str>, default| match value.map(str::trim) {
            None | Some("") => default,
            Some(v) => match v.parse::<u16>() {
                Ok(p) if p != 0 => p,
                _ => {
                    invalid.push((variable, v.to_owned()));
                    default
                }
            },
        };
        let usb_port = port("PADBOARD_PORT", usb_port, USB_PORT);
        let mut wifi_port = port("PADBOARD_WIFI_PORT", wifi_port, WIFI_PORT);
        if wifi_port == usb_port {
            invalid.push(("PADBOARD_WIFI_PORT", wifi_port.to_string()));
            wifi_port = if usb_port == WIFI_PORT {
                USB_PORT
            } else {
                WIFI_PORT
            };
        }

        Self {
            wifi_forced,
            wifi_forced_by_test: false,
            usb_port,
            wifi_port,
            invalid,
        }
    }

    /// USB listener: loopback only.
    #[must_use]
    pub fn usb_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.usb_port)
    }

    /// Wi-Fi listener: every interface.
    #[must_use]
    pub fn wifi_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), self.wifi_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_leave_wifi_to_the_ui_and_usb_on_loopback() {
        let s = Settings::from_vars(None, None, None);
        assert_eq!(s.wifi_forced, None);
        assert!(s.invalid.is_empty());
        assert_eq!(s.usb_addr().to_string(), "127.0.0.1:47321");
        assert_eq!(s.wifi_addr().to_string(), "0.0.0.0:47322");
    }

    #[test]
    fn bind_forces_wifi_on_or_off() {
        for v in ["lan", "LAN", " 0.0.0.0 ", "wifi"] {
            assert_eq!(
                Settings::from_vars(Some(v), None, None).wifi_forced,
                Some(true),
                "{v}"
            );
        }
        for v in ["loopback", "localhost", "127.0.0.1"] {
            assert_eq!(
                Settings::from_vars(Some(v), None, None).wifi_forced,
                Some(false),
                "{v}"
            );
        }
        assert_eq!(Settings::from_vars(Some(""), None, None).wifi_forced, None);
    }

    #[test]
    fn invalid_values_fall_back_to_safe_defaults() {
        let s = Settings::from_vars(Some("192.168.1.10"), Some("99999"), Some("x"));
        assert_eq!(s.wifi_forced, None);
        assert_eq!(s.usb_port, USB_PORT);
        assert_eq!(s.wifi_port, WIFI_PORT);
        assert_eq!(
            s.invalid,
            [
                ("PADBOARD_BIND", "192.168.1.10".to_owned()),
                ("PADBOARD_PORT", "99999".to_owned()),
                ("PADBOARD_WIFI_PORT", "x".to_owned()),
            ]
        );
        assert_eq!(
            Settings::from_vars(None, Some("0"), None).usb_port,
            USB_PORT
        );
        assert_eq!(
            Settings::from_vars(None, Some("50000"), None).usb_port,
            50000
        );
    }

    #[test]
    fn ports_must_differ() {
        let s = Settings::from_vars(None, Some("47322"), None);
        assert_eq!(s.usb_port, 47322);
        assert_eq!(s.wifi_port, USB_PORT);
        assert_eq!(s.invalid, [("PADBOARD_WIFI_PORT", "47322".to_owned())]);
    }
}
