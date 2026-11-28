//! The screen on Linux: the XDG desktop portal's ScreenCast interface
//! (GNOME, KDE… on Wayland and X11), read with GStreamer's `pipewiresrc`.
//!
//! A session asks for a **virtual** monitor (an extended screen, its size
//! decided by the stream's caps; mirroring a monitor would be another
//! source type). The first time, the desktop asks the user (a dialog on the
//! PC); with a restore token (`persist_mode` 2) the next sessions start
//! without asking, until the user revokes it. Closing the session (or the
//! D-Bus connection) removes the virtual monitor.
//!
//! Every portal call answers through a `Request` object's `Response`
//! signal; its path is known in advance (sender + `handle_token`), so it is
//! watched before the call (no race).

use std::collections::HashMap;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SCREEN_CAST: &str = "org.freedesktop.portal.ScreenCast";

/// How long the user has to answer the desktop's dialog.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(120);

/// Source type bit of a virtual monitor (1 monitor, 2 window, 4 virtual).
const VIRTUAL: u32 = 4;

/// Why a session could not start.
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("no screen sharing portal on this desktop ({0})")]
    Unavailable(String),
    #[error("this desktop cannot add a virtual screen")]
    Unsupported,
    #[error("screen sharing refused on the PC")]
    Cancelled,
    #[error("screen sharing not answered on the PC")]
    NoAnswer,
    #[error("screen sharing failed: {0}")]
    Failed(String),
}

impl From<zbus::Error> for PortalError {
    fn from(e: zbus::Error) -> Self {
        Self::Failed(e.to_string())
    }
}

/// A started session: PipeWire's connection and node to read. Dropping it
/// closes the session (a virtual monitor disappears).
pub struct Session {
    _open: Open,
    /// PipeWire remote for `pipewiresrc fd=…` (which duplicates it).
    pub fd: OwnedFd,
    /// The stream's node, `pipewiresrc path=…`.
    pub node: u32,
    /// Token to start the same sharing next time without asking.
    pub restore_token: Option<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("node", &self.node)
            .field(
                "restore_token",
                &self
                    .restore_token
                    .as_ref()
                    .map(|_| padboard_protocol::REDACTED),
            )
            .finish_non_exhaustive()
    }
}

/// A portal session, closed when dropped (also on a failure while it
/// starts).
struct Open {
    connection: Connection,
    handle: OwnedObjectPath,
}

impl Drop for Open {
    fn drop(&mut self) {
        let closed = Proxy::new(
            &self.connection,
            DEST,
            self.handle.as_str(),
            "org.freedesktop.portal.Session",
        )
        .and_then(|p| p.call_method("Close", &()).map(|_| ()));
        if let Err(e) = closed {
            tracing::debug!(error = %e, "second screen: closing the portal session");
        }
    }
}

/// Starts sharing a new virtual monitor, the cursor drawn in the picture. Blocks until
/// the user answers the desktop's dialog (none with a valid `restore`
/// token).
///
/// # Errors
/// No portal, refused or unanswered, or a portal failure.
pub fn start(restore: Option<&str>) -> Result<Session, PortalError> {
    let connection = Connection::session().map_err(|e| PortalError::Unavailable(e.to_string()))?;
    let portal = Proxy::new(&connection, DEST, PATH, SCREEN_CAST)
        .map_err(|e| PortalError::Unavailable(e.to_string()))?;
    let available: u32 = portal
        .get_property("AvailableSourceTypes")
        .map_err(|e| PortalError::Unavailable(e.to_string()))?;
    if available & VIRTUAL == 0 {
        return Err(PortalError::Unsupported);
    }

    let session_token = token();
    let created = request(&connection, |handle_token| {
        let options = HashMap::from([
            ("handle_token", Value::from(handle_token)),
            ("session_handle_token", Value::from(session_token.as_str())),
        ]);
        portal.call_method("CreateSession", &(options,)).map(|_| ())
    })?;
    let handle: String = take(&created, "session_handle")?;
    let handle =
        OwnedObjectPath::try_from(handle).map_err(|e| PortalError::Failed(e.to_string()))?;
    // From here the session exists: closed by Drop on any failure.
    let open = Open {
        connection: connection.clone(),
        handle,
    };

    request(&connection, |handle_token| {
        let mut options = HashMap::from([
            ("handle_token", Value::from(handle_token)),
            ("types", Value::from(VIRTUAL)),
            ("multiple", Value::from(false)),
            // Embedded: the tablet shows the pointer.
            ("cursor_mode", Value::from(2_u32)),
            // Until revoked.
            ("persist_mode", Value::from(2_u32)),
        ]);
        if let Some(restore) = restore {
            options.insert("restore_token", Value::from(restore));
        }
        portal
            .call_method("SelectSources", &(&open.handle, options))
            .map(|_| ())
    })?;

    let started = request(&connection, |handle_token| {
        let options = HashMap::from([("handle_token", Value::from(handle_token))]);
        portal
            .call_method("Start", &(&open.handle, "", options))
            .map(|_| ())
    })?;
    let streams: Vec<(u32, HashMap<String, OwnedValue>)> = take(&started, "streams")?;
    let (node, _) = streams
        .into_iter()
        .next()
        .ok_or_else(|| PortalError::Failed("no stream".into()))?;
    let restore_token = take::<String>(&started, "restore_token").ok();
    let fd: zbus::zvariant::OwnedFd = portal.call(
        "OpenPipeWireRemote",
        &(&open.handle, HashMap::<&str, Value>::new()),
    )?;
    tracing::info!(node, "second screen: virtual screen shared");
    Ok(Session {
        _open: open,
        fd: fd.into(),
        node,
        restore_token,
    })
}

/// A unique token for the portal's object paths.
fn token() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!(
        "padboard_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Calls a portal method through `call` (given the `handle_token` to put
/// in its options) and waits for its answer's results.
fn request(
    connection: &Connection,
    call: impl FnOnce(&str) -> zbus::Result<()>,
) -> Result<HashMap<String, OwnedValue>, PortalError> {
    let handle_token = token();
    let sender = connection
        .unique_name()
        .ok_or_else(|| PortalError::Failed("no D-Bus name".into()))?
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("{PATH}/request/{sender}/{handle_token}");
    let request = Proxy::new(
        connection,
        DEST,
        path.as_str(),
        "org.freedesktop.portal.Request",
    )?;
    let mut responses = request.receive_signal("Response")?;
    call(&handle_token)?;
    // The iterator blocks: wait for it in a thread, with a deadline.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("screen-portal".into())
        .spawn(move || {
            let _ = tx.send(responses.next());
        })
        .map_err(|e| PortalError::Failed(e.to_string()))?;
    let message = match rx.recv_timeout(ANSWER_TIMEOUT) {
        Ok(Some(message)) => message,
        Ok(None) => return Err(PortalError::Failed("the portal went away".into())),
        Err(_) => {
            // Closes the dialog; the waiting thread then ends.
            let _ = request.call_method("Close", &());
            return Err(PortalError::NoAnswer);
        }
    };
    let (response, results): (u32, HashMap<String, OwnedValue>) = message
        .body()
        .deserialize()
        .map_err(|e| PortalError::Failed(e.to_string()))?;
    match response {
        0 => Ok(results),
        1 => Err(PortalError::Cancelled),
        _ => Err(PortalError::Failed("the desktop ended the request".into())),
    }
}

fn take<T>(results: &HashMap<String, OwnedValue>, key: &str) -> Result<T, PortalError>
where
    T: TryFrom<OwnedValue>,
    T::Error: std::fmt::Display,
{
    let value = results
        .get(key)
        .ok_or_else(|| PortalError::Failed(format!("no {key} in the answer")))?;
    T::try_from(
        value
            .try_clone()
            .map_err(|e| PortalError::Failed(e.to_string()))?,
    )
    .map_err(|e| PortalError::Failed(format!("{key}: {e}")))
}
