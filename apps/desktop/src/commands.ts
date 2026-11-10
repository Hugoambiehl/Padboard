// Typed wrappers of the Tauri commands (apps/desktop/src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";
import type { PairingView } from "./types/status";

export const startPairing = () => invoke<PairingView>("start_pairing");
export const cancelPairing = () => invoke<void>("cancel_pairing");
export const revokeDevice = (deviceId: string) => invoke<boolean>("revoke_device", { deviceId });
export const setWifiEnabled = (enabled: boolean) => invoke<void>("set_wifi_enabled", { enabled });
export const setAdbPath = (path: string | null) => invoke<void>("set_adb_path", { path });
