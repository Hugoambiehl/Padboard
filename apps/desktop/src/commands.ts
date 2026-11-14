// Typed wrappers of the Tauri commands (apps/desktop/src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";
import type { ActionOutcome, PairingView } from "./types/status";
import type { Edit, EditOutcome, EditorState } from "./generated/types";
import type { UpdatePreferences, UpdatesView } from "./types/updates";

export const startPairing = () => invoke<PairingView>("start_pairing");
export const cancelPairing = () => invoke<void>("cancel_pairing");
export const revokeDevice = (deviceId: string) => invoke<boolean>("revoke_device", { deviceId });
export const setWifiEnabled = (enabled: boolean) => invoke<void>("set_wifi_enabled", { enabled });
export const setAdbPath = (path: string | null) => invoke<void>("set_adb_path", { path });
export const enableKiosk = (serial: string) => invoke<void>("enable_kiosk", { serial });
/** Installs this PC's version on the USB tablet (`adb install -r`); the version installed. */
export const updateUsbTablet = (serial: string) => invoke<string>("update_usb_tablet", { serial });
/** Sends this PC's version to a connected kiosk tablet; progress comes with the status. */
export const updateWifiTablet = (clientId: number) =>
  invoke<string>("update_wifi_tablet", { clientId });
export const exitKiosk = (clientId: number) => invoke<boolean>("exit_kiosk", { clientId });

/** Screensaver of the tablets (`preferences.rs`). */
export interface Screensaver {
  enabled: boolean;
  delaySeconds: number;
}
export const getScreensaver = () => invoke<Screensaver>("get_screensaver");
export const setScreensaver = (screensaver: Screensaver) =>
  invoke<void>("set_screensaver", { screensaver });

// Editor
export const getEditor = () => invoke<EditorState>("get_editor");
/** Rejects with an `EditError` (`{ code, detail }`) when the edit is refused. */
export const applyEdit = (edit: Edit) => invoke<EditOutcome>("edit", { edit });
export const undo = () => invoke<boolean>("undo");
export const redo = () => invoke<boolean>("redo");
export const checkpoint = () => invoke<void>("checkpoint");
export const listKeys = () => invoke<string[]>("list_keys");
export const testButton = (buttonId: string) => invoke<ActionOutcome>("test_button", { buttonId });

export type ObsStatus =
  | { state: "disabled" }
  | { state: "connecting" }
  | { state: "connected"; version: string }
  | { state: "unreachable"; detail: string }
  | { state: "authFailed" };

/** OBS settings (the password itself never comes back) and state. */
export type ObsInfo =
  | { available: false }
  | {
      available: true;
      enabled: boolean;
      host: string;
      port: number;
      hasPassword: boolean;
      status: ObsStatus;
      scenes: string[];
      inputs: string[];
    };

export const getObs = () => invoke<ObsInfo>("get_obs");
/** `password`: undefined keeps the saved one, "" removes it. */
export const setObs = (enabled: boolean, host: string, port: number, password?: string) =>
  invoke<void>("set_obs", { enabled, host, port, password: password ?? null });

// Window and session
/** Hides the window in the tray (and remembers the first-close explanation was seen). */
export const hideToTray = () => invoke<void>("hide_to_tray");
export const quitApp = () => invoke<void>("quit_app");
/** Start with the session (hidden in the tray). */
export const getAutostart = () => invoke<boolean>("get_autostart");
export const setAutostart = (enabled: boolean) => invoke<void>("set_autostart", { enabled });

// Updates
export const getUpdates = () => invoke<UpdatesView>("get_updates");
export const setUpdatePreferences = (preferences: UpdatePreferences) =>
  invoke<void>("set_update_preferences", { preferences });
export const checkForUpdates = () => invoke<void>("check_for_updates");
/** Downloads, checks and installs the update (Windows); the app then restarts. */
export const installUpdate = () => invoke<void>("install_update");
/** The available version's page (or the releases), in the browser. */
export const openReleasePage = () => invoke<void>("open_release_page");
