// Typed wrappers of the Tauri commands (apps/desktop/src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";
import type { ActionOutcome, PairingView } from "./types/status";
import type { Edit, EditOutcome, EditorState } from "./generated/types";

export const startPairing = () => invoke<PairingView>("start_pairing");
export const cancelPairing = () => invoke<void>("cancel_pairing");
export const revokeDevice = (deviceId: string) => invoke<boolean>("revoke_device", { deviceId });
export const setWifiEnabled = (enabled: boolean) => invoke<void>("set_wifi_enabled", { enabled });
export const setAdbPath = (path: string | null) => invoke<void>("set_adb_path", { path });
export const enableKiosk = (serial: string) => invoke<void>("enable_kiosk", { serial });
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
