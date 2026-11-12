//! Notification area icon, closing to it and starting with the session.
//!
//! The icon's menu opens the editor, shows the connected tablets and quits.
//! Closing the window hides it there instead of quitting (the tablets keep
//! working); the first time, the frontend explains it (`close-to-tray`
//! event, then `hide_to_tray` or `quit_app`). On Linux the icon needs a
//! StatusNotifier host (GNOME: the AppIndicator extension): without one,
//! closing quits as before, so the app is never left running unreachable.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::auth::Transport;
use crate::locale::{Locale, TrayTexts, tray_texts};
use crate::state::ClientInfo;

/// Event asking the frontend to explain closing to the tray (first close).
pub const CLOSE_TO_TRAY_EVENT: &str = "close-to-tray";
/// Argument of the session autostart entry: start hidden in the tray.
pub const HIDDEN_ARG: &str = "--hidden";

const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

/// What the tray shows; kept in Tauri's state to update the status line.
pub struct Tray<R: Runtime> {
    icon: TrayIcon<R>,
    status: MenuItem<R>,
    texts: &'static TrayTexts,
}

impl<R: Runtime> Tray<R> {
    /// Creates the icon (`tray.png`: the 32 px variant, without the halo).
    pub fn create(app: &AppHandle<R>, locale: Locale) -> tauri::Result<Self> {
        #[cfg(windows)]
        dark_menus();
        let texts = tray_texts(locale);
        let open = MenuItem::with_id(app, OPEN_ID, texts.open, true, None::<&str>)?;
        let status = MenuItem::with_id(app, "status", texts.no_tablet, false, None::<&str>)?;
        let quit = MenuItem::with_id(app, QUIT_ID, texts.quit, true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &open,
                &PredefinedMenuItem::separator(app)?,
                &status,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;
        let icon = TrayIconBuilder::with_id("main")
            .icon(tauri::image::Image::from_bytes(include_bytes!(
                "../icons/tray.png"
            ))?)
            // No `title`: on Linux it is a text label next to the icon.
            // Windows names the icon in its settings ("Other system tray
            // icons") after the tooltip it is created with: a fixed one here,
            // the status is added right after.
            .tooltip(texts.app)
            .menu(&menu)
            // Windows: a left click opens the window, the right click the menu.
            // (Linux AppIndicator always opens the menu.)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id().as_ref() {
                OPEN_ID => show_window(app),
                QUIT_ID => {
                    tracing::info!("quit from the tray");
                    app.exit(0);
                }
                _ => {}
            })
            .on_tray_icon_event(|icon, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(icon.app_handle());
                }
            })
            .build(app)?;
        let _ = icon.set_tooltip(Some(tooltip(texts, texts.no_tablet)));
        Ok(Self {
            icon,
            status,
            texts,
        })
    }

    /// Shows the connected tablets (called with every status snapshot).
    pub fn set_clients(&self, clients: &[ClientInfo]) {
        let line = status_line(self.texts, clients);
        if self.status.text().is_ok_and(|current| current == line) {
            return;
        }
        if let Err(e) = self.status.set_text(&line) {
            tracing::warn!("cannot update the tray menu: {e}");
        }
        let _ = self.icon.set_tooltip(Some(tooltip(self.texts, &line)));
    }
}

/// Draws the app's popup menus (the tray menu) dark, like the rest of the
/// app, whatever the Windows theme. The tray menu belongs to a hidden window
/// of `tray-icon` that never asks for dark mode, so tao's `AllowDark` is not
/// enough. uxtheme's `SetPreferredAppMode` (ordinal 135) and
/// `FlushMenuThemes` (136) are not documented but stable since Windows 10
/// 1903 (used by tao itself, Notepad++…); on 1809 ordinal 135 is
/// `AllowDarkModeForApp(bool)`, which the same call turns on harmlessly.
#[cfg(windows)]
fn dark_menus() {
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::core::{PCSTR, w};
    const FORCE_DARK: i32 = 2;
    // SAFETY: uxtheme.dll is a system library; the ordinals are looked up
    // (missing = nothing done) and called with their known signatures.
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        let set = GetProcAddress(uxtheme, PCSTR(135 as *const u8));
        let flush = GetProcAddress(uxtheme, PCSTR(136 as *const u8));
        if let (Some(set), Some(flush)) = (set, flush) {
            let set = std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(i32) -> i32,
            >(set);
            let flush = std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(),
            >(flush);
            set(FORCE_DARK);
            flush();
        }
    }
}

fn tooltip(texts: &TrayTexts, status: &str) -> String {
    format!("{} · {status}", texts.app)
}

/// "Galaxy Tab A · USB", "2 tablets connected" or "No tablet connected".
#[must_use]
pub fn status_line(texts: &TrayTexts, clients: &[ClientInfo]) -> String {
    match clients {
        [] => texts.no_tablet.to_owned(),
        [client] => format!(
            "{} · {}",
            client.device_name,
            match client.transport {
                Transport::Usb => "USB",
                Transport::Wifi => "Wi-Fi",
            }
        ),
        many => texts.tablets.replace("{n}", &many.len().to_string()),
    }
}

/// Brings the editor window back (tray, second launch).
pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Whether a tray icon can be seen. Linux: a StatusNotifier host must be
/// running (the GNOME AppIndicator extension, KDE, XFCE…); Windows always has
/// a notification area.
#[must_use]
pub fn host_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        let found = (|| -> zbus::Result<bool> {
            let connection = zbus::blocking::Connection::session()?;
            let dbus = zbus::blocking::fdo::DBusProxy::new(&connection)?;
            Ok(dbus.name_has_owner("org.kde.StatusNotifierWatcher".try_into()?)?)
        })();
        found.unwrap_or_else(|e| {
            tracing::warn!("cannot look for a tray host: {e}");
            false
        })
    }
    #[cfg(not(target_os = "linux"))]
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(name: &str, transport: Transport) -> ClientInfo {
        ClientInfo {
            id: 1,
            device_id: "d".into(),
            device_name: name.into(),
            transport,
            peer: String::new(),
            connected_at_ms: 0,
            kiosk: false,
        }
    }

    #[test]
    fn status_lines() {
        let fr = tray_texts(Locale::Fr);
        let en = tray_texts(Locale::En);
        assert_eq!(status_line(fr, &[]), "Aucune tablette connectée");
        assert_eq!(status_line(en, &[]), "No tablet connected");
        assert_eq!(
            status_line(fr, &[client("Galaxy Tab A", Transport::Usb)]),
            "Galaxy Tab A · USB"
        );
        let two = [client("A", Transport::Usb), client("B", Transport::Wifi)];
        assert_eq!(status_line(fr, &two), "2 tablettes connectées");
        assert_eq!(status_line(en, &two), "2 tablets connected");
    }
}
