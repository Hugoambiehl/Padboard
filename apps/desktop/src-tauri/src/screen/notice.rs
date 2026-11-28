//! The PC's question about a second screen as a desktop notification
//! (Linux, `org.freedesktop.Notifications`). GNOME on Wayland never lets
//! an app bring its own window to the front (focus stealing prevention):
//! the window's dialog alone went unseen. The notification is critical
//! (it stays), answers with its buttons, and opens the window when
//! clicked.

use std::collections::HashMap;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// What the PC's user did with the notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Allow,
    Refuse,
    /// Clicked: show the window.
    Open,
}

/// The texts, in the PC's language.
#[derive(Debug, Clone, Copy)]
pub struct Texts<'a> {
    pub app: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub allow: &'a str,
    pub refuse: &'a str,
    pub open: &'a str,
}

/// A notification on screen; [`Notice::close`] removes it.
pub struct Notice {
    connection: Connection,
    id: u32,
}

impl std::fmt::Debug for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notice")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Notice {
    /// Shows the question; `on_choice` is called (from a thread of its own)
    /// for each button or click, until the notification closes.
    ///
    /// # Errors
    /// No notification server (a desktop without one, the e2e tests'
    /// virtual display).
    pub fn show(
        texts: Texts<'_>,
        on_choice: impl Fn(Choice) + Send + 'static,
    ) -> zbus::Result<Self> {
        let connection = Connection::session()?;
        let proxy = Proxy::new(&connection, DEST, PATH, DEST)?;
        // Watched before showing it: no click can be missed.
        let signals = proxy.receive_all_signals()?;
        let actions = [
            "default",
            texts.open,
            "allow",
            texts.allow,
            "refuse",
            texts.refuse,
        ];
        // Critical: it stays until answered (the question times out on its own).
        let hints = HashMap::from([("urgency", Value::U8(2))]);
        let id: u32 = proxy.call(
            "Notify",
            &(
                texts.app,
                0_u32,
                "",
                texts.title,
                texts.body,
                &actions[..],
                hints,
                0_i32,
            ),
        )?;
        std::thread::Builder::new()
            .name("screen-notice".into())
            .spawn(move || {
                for message in signals {
                    let header = message.header();
                    let member = header
                        .member()
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default();
                    match member.as_str() {
                        "ActionInvoked" => {
                            let Ok((of, key)) = message.body().deserialize::<(u32, String)>()
                            else {
                                continue;
                            };
                            if of != id {
                                continue;
                            }
                            match key.as_str() {
                                "allow" => on_choice(Choice::Allow),
                                "refuse" => on_choice(Choice::Refuse),
                                _ => on_choice(Choice::Open),
                            }
                        }
                        "NotificationClosed"
                            if message
                                .body()
                                .deserialize::<(u32, u32)>()
                                .is_ok_and(|(of, _)| of == id) =>
                        {
                            break;
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|e| zbus::Error::Failure(e.to_string()))?;
        Ok(Self { connection, id })
    }

    /// Removes it (answered in the window, or timed out).
    pub fn close(&self) {
        let closed = Proxy::new(&self.connection, DEST, PATH, DEST)
            .and_then(|p| p.call_method("CloseNotification", &(self.id,)).map(|_| ()));
        if let Err(e) = closed {
            tracing::debug!(error = %e, "second screen: closing the notification");
        }
    }
}
