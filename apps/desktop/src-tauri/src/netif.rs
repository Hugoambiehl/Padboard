//! Network interfaces: which ones are physical (Wi-Fi, Ethernet) and the
//! addresses shown in the UI.
//!
//! Virtual interfaces (loopback, Docker, bridges, VMs, VPNs) are never
//! announced over mDNS and are listed after the physical ones: a tablet can
//! only reach the PC through the local network it shares with it.

use std::collections::BTreeSet;
use std::net::IpAddr;

use serde::Serialize;

/// Name prefixes of virtual interfaces (lowercase).
const VIRTUAL_PREFIXES: &[&str] = &[
    // Linux
    "docker",
    "br-",
    "veth",
    "virbr",
    "tailscale",
    "vmnet",
    "vboxnet",
    "wg",
    "tun",
    "tap",
    "zt",
    "utun", // Windows (friendly names)
    "vethernet",
    "loopback",
    "virtualbox",
    "vmware",
];

/// `true` for loopback, containers, bridges, VMs and VPNs.
#[must_use]
pub fn is_virtual(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "lo" || VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// An IPv4 address of this PC, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanAddress {
    pub address: String,
    pub interface: String,
    /// Wi-Fi or Ethernet (where tablets can reach the PC).
    pub physical: bool,
}

/// The current interfaces: addresses for the UI (physical first) and the
/// names of the virtual interfaces (excluded from mDNS).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interfaces {
    pub addresses: Vec<LanAddress>,
    pub virtual_names: BTreeSet<String>,
}

#[must_use]
pub fn scan() -> Interfaces {
    match if_addrs::get_if_addrs() {
        Ok(interfaces) => classify(
            interfaces
                .into_iter()
                .map(|i| (i.name.clone(), i.ip(), i.is_loopback())),
        ),
        Err(e) => {
            tracing::warn!("cannot list network interfaces: {e}");
            Interfaces::default()
        }
    }
}

fn classify(interfaces: impl Iterator<Item = (String, IpAddr, bool)>) -> Interfaces {
    let mut result = Interfaces::default();
    for (name, ip, loopback) in interfaces {
        let physical = !loopback && !is_virtual(&name);
        if !physical {
            result.virtual_names.insert(name.clone());
        }
        if let IpAddr::V4(v4) = ip
            && !v4.is_loopback()
            && !v4.is_link_local()
        {
            result.addresses.push(LanAddress {
                address: v4.to_string(),
                interface: name,
                physical,
            });
        }
    }
    // Physical first, then by interface name and address.
    result.addresses.sort_by(|a, b| {
        (!a.physical, &a.interface, &a.address).cmp(&(!b.physical, &b.interface, &b.address))
    });
    result.addresses.dedup();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_virtual_interfaces() {
        for name in [
            "lo",
            "docker0",
            "br-568a00b37a31",
            "veth3efa70a",
            "virbr0",
            "tailscale0",
            "wg0",
            "vEthernet (WSL)",
            "Loopback Pseudo-Interface 1",
            "VirtualBox Host-Only Network",
        ] {
            assert!(is_virtual(name), "{name}");
        }
        for name in ["wlo1", "wlan0", "eth0", "enp3s0", "Wi-Fi", "Ethernet"] {
            assert!(!is_virtual(name), "{name}");
        }
    }

    #[test]
    fn physical_addresses_come_first() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let found = classify(
            [
                ("br-568a".to_owned(), ip("172.18.0.1"), false),
                ("tailscale0".to_owned(), ip("100.75.18.112"), false),
                ("wlo1".to_owned(), ip("192.168.1.42"), false),
                ("wlo1".to_owned(), ip("fe80::1"), false),
                ("lo".to_owned(), ip("127.0.0.1"), true),
            ]
            .into_iter(),
        );
        let order: Vec<(&str, bool)> = found
            .addresses
            .iter()
            .map(|a| (a.address.as_str(), a.physical))
            .collect();
        assert_eq!(
            order,
            [
                ("192.168.1.42", true),
                ("172.18.0.1", false),
                ("100.75.18.112", false)
            ]
        );
        assert_eq!(
            found.virtual_names,
            ["br-568a", "lo", "tailscale0"]
                .into_iter()
                .map(String::from)
                .collect()
        );
    }
}
