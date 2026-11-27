//! The TCP table on Linux: `/proc/net/tcp` and `tcp6` give every socket
//! of the network namespace with its owner's UID and its inode, whatever
//! the account; the process owning an inode is found among the processes
//! whose descriptors we may read (`/proc/<pid>/fd/* → socket:[inode]`):
//! ours, not root's or another account's.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::{TcpRow, TcpState};

/// Both tables; `None` if neither can be read.
pub fn read_tables() -> Option<Vec<TcpRow>> {
    let v4 = std::fs::read_to_string("/proc/net/tcp").ok();
    let v6 = std::fs::read_to_string("/proc/net/tcp6").ok();
    if v4.is_none() && v6.is_none() {
        return None;
    }
    let mut rows = v4.map(|t| parse(&t, false)).unwrap_or_default();
    rows.extend(v6.map(|t| parse(&t, true)).unwrap_or_default());
    Some(rows)
}

/// Parses one table (`v6`: `/proc/net/tcp6`). Unreadable lines are skipped.
pub fn parse(text: &str, v6: bool) -> Vec<TcpRow> {
    text.lines()
        .skip(1)
        .filter_map(|l| parse_line(l, v6))
        .collect()
}

fn parse_line(line: &str, v6: bool) -> Option<TcpRow> {
    // sl local_address rem_address st tx:rx tr:when retrnsmt uid timeout inode
    let fields: Vec<&str> = line.split_whitespace().collect();
    let local = address(fields.get(1)?, v6)?;
    let (_, remote_port) = fields.get(2)?.split_once(':')?;
    let state = match *fields.get(3)? {
        "01" => TcpState::Established,
        "0A" => TcpState::Listen,
        _ => TcpState::Other,
    };
    Some(TcpRow {
        state,
        local,
        remote_port: u16::from_str_radix(remote_port, 16).ok()?,
        uid: fields.get(7)?.parse().ok(),
        inode: fields.get(9)?.parse().ok().filter(|&i| i != 0),
        pid: None,
    })
}

/// `0100007F:CC5B` (IPv4: one 32-bit word in the kernel's byte order, i.e.
/// little-endian here) or 32 hex digits (IPv6: four such words).
fn address(text: &str, v6: bool) -> Option<SocketAddr> {
    let (ip, port) = text.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let word = |i: usize| -> Option<[u8; 4]> {
        let hex = ip.get(8 * i..8 * i + 8)?;
        Some(u32::from_str_radix(hex, 16).ok()?.to_le_bytes())
    };
    let ip = if v6 {
        if ip.len() != 32 {
            return None;
        }
        let mut bytes = [0u8; 16];
        for i in 0..4 {
            bytes[4 * i..4 * i + 4].copy_from_slice(&word(i)?);
        }
        IpAddr::V6(Ipv6Addr::from(bytes))
    } else {
        if ip.len() != 8 {
            return None;
        }
        IpAddr::V4(Ipv4Addr::from(word(0)?))
    };
    Some(SocketAddr::new(ip, port))
}

/// The processes owning these socket inodes, among those we may read.
pub fn pids_of(inodes: &[u64]) -> HashMap<u64, u32> {
    let mut found = HashMap::new();
    if inodes.is_empty() {
        return found;
    }
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return found;
    };
    for proc in procs.flatten() {
        let Some(pid) = proc
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        // Another account's process: not readable, skipped.
        let Ok(fds) = std::fs::read_dir(proc.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            if let Some(inode) = socket_inode(&target.to_string_lossy())
                && inodes.contains(&inode)
            {
                found.insert(inode, pid);
            }
        }
        if found.len() == inodes.len() {
            break;
        }
    }
    found
}

fn socket_inode(link: &str) -> Option<u64> {
    link.strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lines taken from this PC on 2026-11-27, with the tablet connected
    // through `adb reverse` (the adb server, uid 1000, owns both inodes).
    const TCP: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:13AD 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 61190 1 0000000000000000 100 0 0 10 0
   1: 0100007F:CC5B 0100007F:B8D9 01 00000000:00000000 00:00000000 00000000  1000        0 79405 1 0000000000000000 20 4 30 10 -1
   2: 0100007F:B8D9 0100007F:CC5B 01 00000000:00000000 00:00000000 00000000  1000        0 79406 1 0000000000000000 20 4 30 10 -1
   3: 0100007F:9C40 0100007F:13AD 06 00000000:00000000 03:00000000 00000000     0        0 0 3 0000000000000000
";
    const TCP6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000001000000:13AD 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 5555 1 0000000000000000 100 0 0 10 0
   1: 0000000000000000FFFF00000100007F:CC5C 0000000000000000FFFF00000100007F:B8D9 01 00000000:00000000 00:00000000 00000000  1001        0 777 1 0000000000000000 20 4 30 10 -1
";

    #[test]
    fn ipv4_rows_are_read() {
        let rows = parse(TCP, false);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].state, TcpState::Listen);
        assert_eq!(rows[0].local, "127.0.0.1:5037".parse().unwrap());
        assert_eq!(rows[0].uid, Some(1000));
        assert_eq!(rows[0].inode, Some(61190));
        // The adb server's end of the tablet's connection to 47321.
        assert_eq!(rows[1].state, TcpState::Established);
        assert_eq!(rows[1].local, "127.0.0.1:52315".parse().unwrap());
        assert_eq!(rows[1].remote_port, 47321);
        assert_eq!(rows[1].inode, Some(79405));
        // TIME_WAIT: no owner, no inode.
        assert_eq!(rows[3].state, TcpState::Other);
        assert_eq!(rows[3].inode, None);
    }

    #[test]
    fn ipv6_rows_are_read() {
        let rows = parse(TCP6, true);
        assert_eq!(rows[0].local, "[::1]:5037".parse().unwrap());
        assert_eq!(rows[0].uid, Some(0));
        assert_eq!(rows[1].local, "[::ffff:127.0.0.1]:52316".parse().unwrap());
        assert_eq!(rows[1].remote_port, 47321);
    }

    #[test]
    fn broken_lines_are_skipped() {
        let rows = parse("header\n garbage\n 0: 0100007F 0100007F:0000 0A\n", false);
        assert!(rows.is_empty());
    }

    #[test]
    fn socket_links_are_read() {
        assert_eq!(socket_inode("socket:[79405]"), Some(79405));
        assert_eq!(socket_inode("pipe:[12]"), None);
        assert_eq!(socket_inode("/dev/null"), None);
    }

    #[test]
    fn this_process_owns_its_own_sockets() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let rows = read_tables().unwrap();
        let row = rows
            .iter()
            .find(|r| r.state == TcpState::Listen && r.local.port() == port)
            .unwrap();
        let pids = pids_of(&[row.inode.unwrap()]);
        assert_eq!(pids.get(&row.inode.unwrap()), Some(&std::process::id()));
    }
}
