//! Server settings read from the environment.
//!
//! - `PADBOARD_BIND`: `loopback` (default, also `localhost` / `127.0.0.1`) or
//!   `lan` (also `0.0.0.0`). Until pairing exists (phase 2), the server is
//!   unauthenticated, so it only listens on the loopback interface by default:
//!   enough for USB through `adb reverse`. LAN listening is an explicit opt-in.
//! - `PADBOARD_PORT`: TCP port, default [`DEFAULT_PORT`].
//!
//! Invalid values never widen exposure: they fall back to the defaults and are
//! reported to the UI.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::Serialize;

pub const DEFAULT_PORT: u16 = 47321;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BindMode {
    /// 127.0.0.1 only (USB via `adb reverse`).
    Loopback,
    /// Every interface (Wi-Fi). Unauthenticated until pairing exists.
    Lan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub bind: BindMode,
    pub port: u16,
    /// Environment values that were ignored, as `(variable, value)`.
    pub invalid: Vec<(&'static str, String)>,
}

impl Settings {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_vars(
            std::env::var("PADBOARD_BIND").ok().as_deref(),
            std::env::var("PADBOARD_PORT").ok().as_deref(),
        )
    }

    #[must_use]
    pub fn from_vars(bind: Option<&str>, port: Option<&str>) -> Self {
        let mut invalid = Vec::new();

        let bind = match bind.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("" | "loopback" | "localhost" | "127.0.0.1") => BindMode::Loopback,
            Some("lan" | "0.0.0.0") => BindMode::Lan,
            Some(_) => {
                invalid.push(("PADBOARD_BIND", bind.unwrap_or_default().to_owned()));
                BindMode::Loopback
            }
        };

        let port = match port.map(str::trim) {
            None | Some("") => DEFAULT_PORT,
            Some(v) => match v.parse::<u16>() {
                Ok(p) if p != 0 => p,
                _ => {
                    invalid.push(("PADBOARD_PORT", v.to_owned()));
                    DEFAULT_PORT
                }
            },
        };

        Self {
            bind,
            port,
            invalid,
        }
    }

    #[must_use]
    pub fn socket_addr(&self) -> SocketAddr {
        let ip = match self.bind {
            BindMode::Loopback => IpAddr::V4(Ipv4Addr::LOCALHOST),
            BindMode::Lan => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        };
        SocketAddr::new(ip, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_loopback_and_default_port() {
        let s = Settings::from_vars(None, None);
        assert_eq!(s.bind, BindMode::Loopback);
        assert_eq!(s.port, DEFAULT_PORT);
        assert!(s.invalid.is_empty());
        assert_eq!(s.socket_addr().to_string(), "127.0.0.1:47321");
    }

    #[test]
    fn lan_is_explicit_opt_in() {
        for v in ["lan", "LAN", " 0.0.0.0 "] {
            let s = Settings::from_vars(Some(v), None);
            assert_eq!(s.bind, BindMode::Lan, "{v}");
            assert_eq!(s.socket_addr().to_string(), "0.0.0.0:47321");
        }
        for v in ["loopback", "localhost", "127.0.0.1", ""] {
            assert_eq!(
                Settings::from_vars(Some(v), None).bind,
                BindMode::Loopback,
                "{v}"
            );
        }
    }

    #[test]
    fn invalid_values_fall_back_to_safe_defaults() {
        let s = Settings::from_vars(Some("192.168.1.10"), Some("99999"));
        assert_eq!(s.bind, BindMode::Loopback);
        assert_eq!(s.port, DEFAULT_PORT);
        assert_eq!(
            s.invalid,
            [
                ("PADBOARD_BIND", "192.168.1.10".to_owned()),
                ("PADBOARD_PORT", "99999".to_owned())
            ]
        );
        assert_eq!(Settings::from_vars(None, Some("0")).port, DEFAULT_PORT);
        assert_eq!(Settings::from_vars(None, Some("50000")).port, 50000);
    }
}
