//! Who is on the other end of a connection to the USB port (#20,
//! `docs/design/second-screen.md`, SECURITY.md).
//!
//! The USB port listens on loopback, so any local process could reach it.
//! A real USB connection always comes from the **adb server**: `adb reverse`
//! makes it open the connection on the PC's side. Each connection is
//! therefore checked once, when accepted: the process owning its other end
//! must be the one listening on the adb server's port (5037, or
//! `ANDROID_ADB_SERVER_PORT`).
//!
//! - Both processes known (Windows always; Linux when the server runs under
//!   our account): same PID = the adb server.
//! - Otherwise (Linux, a server started by root or another account: its
//!   descriptors are hidden from us): the socket owners' UIDs, which
//!   `/proc/net/tcp` always gives, must be equal.
//! - Whatever else cannot be told (no adb server listening, the connection
//!   gone from the table, a table that cannot be read) is *unverifiable*:
//!   refused too, but explained on the PC and on the tablet.

#[cfg(test)]
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;

/// Which connections the USB port accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPeers {
    /// Only connections opened by the adb server listening on this port.
    AdbServer { port: u16 },
    /// Any local process: simulated test mode (`fake-tablet`, e2e) and unit
    /// tests, which connect directly.
    Any,
}

/// State of a row of the system's TCP table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Listen,
    Established,
    Other,
}

/// A row of the system's TCP table, from either system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpRow {
    pub state: TcpState,
    pub local: SocketAddr,
    pub remote_port: u16,
    /// Owner account (Linux).
    pub uid: Option<u32>,
    /// Socket inode (Linux), to find its process.
    pub inode: Option<u64>,
    /// Owning process (Windows; Linux once resolved from the inode).
    pub pid: Option<u32>,
}

/// What is known of one end: its process and account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Owner {
    pub pid: Option<u32>,
    pub uid: Option<u32>,
}

impl From<&TcpRow> for Owner {
    fn from(row: &TcpRow) -> Self {
        Self {
            pid: row.pid,
            uid: row.uid,
        }
    }
}

/// Why a connection could not be checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unverifiable {
    /// Nothing listens on the adb server's port.
    NoAdbServer,
    /// The connection is not in the system's table (closed meanwhile).
    PeerNotFound,
    /// Neither the processes nor the accounts of both ends are known.
    OwnersUnknown,
    /// The system's table cannot be read.
    TableUnreadable,
}

/// The outcome of the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Opened by the adb server: a device through `adb reverse`.
    AdbServer,
    /// Opened by another process: refused.
    NotAdb,
    /// Could not be told: refused, with an explanation.
    Unverifiable(Unverifiable),
}

/// The decision, from what is known of both ends.
#[must_use]
pub fn decide(peer: Option<Owner>, server: Option<Owner>) -> Verdict {
    let Some(server) = server else {
        return Verdict::Unverifiable(Unverifiable::NoAdbServer);
    };
    let Some(peer) = peer else {
        return Verdict::Unverifiable(Unverifiable::PeerNotFound);
    };
    let same = match (peer.pid, server.pid, peer.uid, server.uid) {
        (Some(a), Some(b), _, _) => a == b,
        (_, _, Some(a), Some(b)) => a == b,
        _ => return Verdict::Unverifiable(Unverifiable::OwnersUnknown),
    };
    if same {
        Verdict::AdbServer
    } else {
        Verdict::NotAdb
    }
}

/// The connection from `peer` to our `usb_port`, and the adb server
/// listening on `adb_port`, in the table.
#[must_use]
pub fn find(
    rows: &[TcpRow],
    peer: SocketAddr,
    usb_port: u16,
    adb_port: u16,
) -> (Option<&TcpRow>, Option<&TcpRow>) {
    let canonical = |a: SocketAddr| (a.ip().to_canonical(), a.port());
    let peer_row = rows.iter().find(|r| {
        r.state == TcpState::Established
            && r.remote_port == usb_port
            && canonical(r.local) == canonical(peer)
    });
    let server_row = rows.iter().find(|r| {
        r.state == TcpState::Listen
            && r.local.port() == adb_port
            && (r.local.ip().is_loopback() || r.local.ip().is_unspecified())
    });
    (peer_row, server_row)
}

/// The full result, for the log and the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub verdict: Verdict,
    /// The peer's process and program, when known.
    pub peer: Owner,
    pub program: Option<PathBuf>,
}

/// A refused USB connection, shown in the USB card.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbRefusal {
    pub at_ms: u64,
    pub pid: Option<u32>,
    pub program: Option<String>,
    /// Absent: another process than the adb server. Present: why it could
    /// not be told (the tablet was told too).
    pub unverifiable: Option<Unverifiable>,
}

impl Check {
    /// What the UI shows of a refused connection.
    #[must_use]
    pub fn refusal(&self, at_ms: u64) -> UsbRefusal {
        UsbRefusal {
            at_ms,
            pid: self.peer.pid,
            program: self.program.as_ref().map(|p| p.display().to_string()),
            unverifiable: match self.verdict {
                Verdict::Unverifiable(why) => Some(why),
                _ => None,
            },
        }
    }
}

/// Checks the connection from `peer` to `usb_port`. Blocking (reads the
/// system's tables): call it from `spawn_blocking`.
#[must_use]
pub fn check(peer: SocketAddr, usb_port: u16, adb_port: u16) -> Check {
    let Some(rows) = table(peer, usb_port, adb_port) else {
        return Check {
            verdict: Verdict::Unverifiable(Unverifiable::TableUnreadable),
            peer: Owner::default(),
            program: None,
        };
    };
    let (peer_row, server_row) = find(&rows, peer, usb_port, adb_port);
    let peer_owner = peer_row.map(Owner::from);
    let verdict = decide(peer_owner, server_row.map(Owner::from));
    let peer_owner = peer_owner.unwrap_or_default();
    Check {
        verdict,
        peer: peer_owner,
        program: peer_owner.pid.and_then(program),
    }
}

/// The rows needed, processes resolved.
#[cfg(target_os = "linux")]
fn table(peer: SocketAddr, usb_port: u16, adb_port: u16) -> Option<Vec<TcpRow>> {
    let mut rows = linux::read_tables()?;
    let (peer_row, server_row) = find(&rows, peer, usb_port, adb_port);
    let inodes: Vec<u64> = [peer_row, server_row]
        .into_iter()
        .flatten()
        .filter_map(|r| r.inode)
        .collect();
    let pids = linux::pids_of(&inodes);
    for row in &mut rows {
        if let Some(inode) = row.inode {
            row.pid = pids.get(&inode).copied();
        }
    }
    Some(rows)
}

#[cfg(windows)]
fn table(_peer: SocketAddr, _usb_port: u16, _adb_port: u16) -> Option<Vec<TcpRow>> {
    crate::platform_tools::server_owner::tcp_rows()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn table(_peer: SocketAddr, _usb_port: u16, _adb_port: u16) -> Option<Vec<TcpRow>> {
    None
}

/// The program of a process, when the system tells it.
#[cfg(target_os = "linux")]
fn program(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

#[cfg(windows)]
fn program(pid: u32) -> Option<PathBuf> {
    crate::platform_tools::server_owner::process_path(pid).ok()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn program(_pid: u32) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loopback(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port)
    }

    const USB: u16 = 47321;
    const ADB: u16 = 5037;

    fn owner(pid: Option<u32>, uid: Option<u32>) -> Option<Owner> {
        Some(Owner { pid, uid })
    }

    #[test]
    fn the_adb_server_is_accepted() {
        // Windows, or Linux under our account: the same process.
        assert_eq!(
            decide(owner(Some(42), Some(1000)), owner(Some(42), Some(1000))),
            Verdict::AdbServer
        );
        // Another program's server (Touch Portal, the SDK's): still the
        // one listening on 5037.
        assert_eq!(
            decide(owner(Some(7), None), owner(Some(7), None)),
            Verdict::AdbServer
        );
    }

    #[test]
    fn another_process_is_refused() {
        // A program of ours, not the server.
        assert_eq!(
            decide(owner(Some(43), Some(1000)), owner(Some(42), Some(1000))),
            Verdict::NotAdb
        );
        // Another account's program (Linux: its process is hidden).
        assert_eq!(
            decide(owner(None, Some(1001)), owner(Some(42), Some(1000))),
            Verdict::NotAdb
        );
    }

    #[test]
    fn a_server_run_by_root_is_checked_by_account() {
        // `sudo adb start-server`: neither process is visible, both
        // sockets belong to root.
        assert_eq!(
            decide(owner(None, Some(0)), owner(None, Some(0))),
            Verdict::AdbServer
        );
        // Our own program while root runs the server.
        assert_eq!(
            decide(owner(Some(43), Some(1000)), owner(None, Some(0))),
            Verdict::NotAdb
        );
    }

    #[test]
    fn what_cannot_be_told_is_unverifiable() {
        assert_eq!(
            decide(owner(Some(1), Some(1)), None),
            Verdict::Unverifiable(Unverifiable::NoAdbServer)
        );
        assert_eq!(
            decide(None, owner(Some(1), Some(1))),
            Verdict::Unverifiable(Unverifiable::PeerNotFound)
        );
        assert_eq!(
            decide(owner(Some(1), None), owner(None, Some(0))),
            Verdict::Unverifiable(Unverifiable::OwnersUnknown)
        );
    }

    fn row(state: TcpState, local: SocketAddr, remote_port: u16, pid: u32) -> TcpRow {
        TcpRow {
            state,
            local,
            remote_port,
            uid: Some(1000),
            inode: None,
            pid: Some(pid),
        }
    }

    #[test]
    fn the_connection_and_the_server_are_found_in_the_table() {
        let peer = loopback(52315);
        let rows = vec![
            // Our own end of the connection (local port = USB).
            row(TcpState::Established, loopback(USB), 52315, 1),
            row(TcpState::Listen, loopback(USB), 0, 1),
            // The adb server: listening, and the client end.
            row(TcpState::Listen, loopback(ADB), 0, 42),
            row(TcpState::Established, peer, USB, 42),
            // Another connection to 5037 (a command).
            row(TcpState::Established, loopback(40000), ADB, 9),
        ];
        let (p, s) = find(&rows, peer, USB, ADB);
        assert_eq!(p.and_then(|r| r.pid), Some(42));
        assert_eq!(s.and_then(|r| r.pid), Some(42));
        assert_eq!(find(&rows, loopback(1), USB, ADB).0, None);
        // A server bound elsewhere than loopback / any is not adb's.
        let lan = SocketAddr::new(IpAddr::from([192, 168, 1, 2]), ADB);
        let only_lan = vec![row(TcpState::Listen, lan, 0, 5)];
        assert_eq!(find(&only_lan, peer, USB, ADB).1, None);
    }

    #[test]
    fn an_ipv4_mapped_row_matches_the_ipv4_peer() {
        let mapped = SocketAddr::new("::ffff:127.0.0.1".parse().unwrap(), 52315);
        let rows = vec![row(TcpState::Established, mapped, USB, 42)];
        assert!(find(&rows, loopback(52315), USB, ADB).0.is_some());
    }
}
