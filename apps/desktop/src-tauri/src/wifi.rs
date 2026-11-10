//! Wi-Fi transport: the TLS listener on all interfaces plus the mDNS
//! announcement, started and stopped together at runtime.
//!
//! Stopping aborts the listener task, which owns its connection tasks: every
//! Wi-Fi connection is closed at once.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mdns_sd::{IfKind, ServiceDaemon, ServiceInfo};
use padboard_protocol::{MDNS_SERVICE_TYPE, PROTOCOL_VERSION, txt};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio_rustls::TlsAcceptor;

use crate::auth::Transport;
use crate::server::{self, ServerContext, ServerEvent};

/// State of the mDNS announcement, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum MdnsStatus {
    Off,
    Announced,
    Error { detail: String },
}

pub struct Wifi {
    ctx: Arc<ServerContext>,
    addr: SocketAddr,
    /// `None` when the TLS identity could not be created: Wi-Fi cannot start.
    tls: Option<TlsAcceptor>,
    runtime: Mutex<Runtime>,
}

#[derive(Default)]
struct Runtime {
    listener: Option<JoinHandle<()>>,
    mdns: Option<ServiceDaemon>,
    announced: Option<String>,
    /// Interfaces never used for mDNS (see `netif.rs`).
    excluded: BTreeSet<String>,
}

impl std::fmt::Debug for Wifi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wifi")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl Wifi {
    #[must_use]
    pub fn new(ctx: Arc<ServerContext>, addr: SocketAddr, tls: Option<TlsAcceptor>) -> Self {
        Self {
            ctx,
            addr,
            tls,
            runtime: Mutex::new(Runtime::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Runtime> {
        self.runtime.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts or stops the listener and the announcement. Idempotent.
    pub fn set_running(&self, on: bool) {
        if on {
            self.start();
        } else {
            self.stop();
        }
    }

    fn start(&self) {
        let Some(tls) = self.tls.clone() else {
            tracing::error!("Wi-Fi cannot start without its TLS identity");
            return;
        };
        let mut runtime = self.lock();
        // A listener that ended (bind failure) may be restarted.
        if runtime
            .listener
            .as_ref()
            .is_some_and(|l| !l.inner().is_finished())
        {
            return;
        }
        let ctx = Arc::clone(&self.ctx);
        let addr = self.addr;
        runtime.listener = Some(tauri::async_runtime::spawn(async move {
            server::run(addr, Transport::Wifi, Some(tls), ctx).await;
        }));
        let status = self.announce(&mut runtime);
        drop(runtime);
        self.emit(ServerEvent::Mdns(status));
    }

    fn stop(&self) {
        let mut runtime = self.lock();
        let Some(listener) = runtime.listener.take() else {
            return;
        };
        listener.abort();
        let announced = runtime.announced.take();
        if let (Some(daemon), Some(fullname)) = (&runtime.mdns, announced)
            && let Err(e) = daemon.unregister(&fullname)
        {
            tracing::warn!("cannot withdraw the mDNS announcement: {e}");
        }
        drop(runtime);
        tracing::info!("Wi-Fi stopped");
        self.emit(ServerEvent::Stopped {
            transport: Transport::Wifi,
        });
        self.emit(ServerEvent::Mdns(MdnsStatus::Off));
    }

    /// Announces `_padboard._tcp` so tablets find the PC without typing its IP.
    fn announce(&self, runtime: &mut Runtime) -> MdnsStatus {
        if runtime.mdns.is_none() {
            match ServiceDaemon::new() {
                Ok(daemon) => {
                    exclude(&daemon, runtime.excluded.iter().cloned().collect());
                    runtime.mdns = Some(daemon);
                }
                Err(e) => {
                    tracing::warn!("mDNS unavailable: {e}");
                    return MdnsStatus::Error {
                        detail: e.to_string(),
                    };
                }
            }
        }
        let Some(daemon) = &runtime.mdns else {
            return MdnsStatus::Off;
        };
        let properties = HashMap::from([
            (
                txt::PROTOCOL_VERSION.to_owned(),
                PROTOCOL_VERSION.to_string(),
            ),
            (txt::SERVER_ID.to_owned(), self.ctx.server_id.clone()),
            (txt::SERVER_NAME.to_owned(), self.ctx.server_name.clone()),
        ]);
        let host = dns_label(&self.ctx.server_name);
        // Unique on the LAN even if two PCs share a hostname.
        let id_prefix: String = self.ctx.server_id.chars().take(8).collect();
        let instance = format!("{host}-{id_prefix}");
        let info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance,
            &format!("{host}.local."),
            (),
            self.addr.port(),
            properties,
        )
        .map(ServiceInfo::enable_addr_auto)
        .and_then(|info| {
            let fullname = info.get_fullname().to_owned();
            daemon.register(info).map(|()| fullname)
        });
        match info {
            Ok(fullname) => {
                tracing::info!(%fullname, "announced over mDNS");
                runtime.announced = Some(fullname);
                MdnsStatus::Announced
            }
            Err(e) => {
                tracing::warn!("cannot announce over mDNS: {e}");
                MdnsStatus::Error {
                    detail: e.to_string(),
                }
            }
        }
    }

    /// Keeps virtual interfaces out of mDNS. Called with every interface scan:
    /// newly seen virtual interfaces (e.g. Docker started later) are excluded
    /// as they appear; physical ones stay enabled (the default).
    pub fn exclude_interfaces(&self, virtual_names: &BTreeSet<String>) {
        let mut runtime = self.lock();
        let new: Vec<String> = virtual_names
            .difference(&runtime.excluded)
            .cloned()
            .collect();
        if new.is_empty() {
            return;
        }
        runtime.excluded.extend(new.iter().cloned());
        if let Some(daemon) = &runtime.mdns {
            exclude(daemon, new);
        }
    }

    fn emit(&self, event: ServerEvent) {
        let _ = self.ctx.events.send(event);
    }
}

impl Drop for Wifi {
    fn drop(&mut self) {
        if let Some(daemon) = self.lock().mdns.take() {
            let _ = daemon.shutdown();
        }
    }
}

fn exclude(daemon: &ServiceDaemon, names: Vec<String>) {
    let mut kinds = vec![IfKind::LoopbackV4, IfKind::LoopbackV6];
    kinds.extend(names.into_iter().map(IfKind::Name));
    if let Err(e) = daemon.disable_interface(kinds) {
        tracing::warn!("cannot exclude interfaces from mDNS: {e}");
    }
}

/// A valid DNS label from a hostname: ASCII letters, digits and `-`, at most
/// 40 characters (the instance name adds a suffix).
fn dns_label(name: &str) -> String {
    let label: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(40)
        .collect();
    let label = label.trim_matches('-');
    if label.is_empty() {
        "padboard".to_owned()
    } else {
        label.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_labels_are_sanitized() {
        assert_eq!(dns_label("My-Laptop-15"), "My-Laptop-15");
        assert_eq!(dns_label("PC de Hugo"), "PC-de-Hugo");
        assert_eq!(dns_label("Élise.local"), "lise-local");
        assert_eq!(dns_label("***"), "padboard");
        assert_eq!(dns_label(&"a".repeat(100)).len(), 40);
    }
}
