//! Updates: the desktop app's own (`tauri-plugin-updater` on Windows, a
//! notice on Linux) and the tablets' (the APK of the same release, pushed by
//! this PC). Everything downloaded is checked against signatures made on the
//! maintainer's PC ([`signing`]).

pub mod assets;
pub mod signing;
