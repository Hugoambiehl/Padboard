// Hand-written mirror of apps/desktop/src-tauri/src/state.rs (UI state, not the
// tablet protocol). Keep both in sync.

import type { UpdateOutcome } from "../generated/types";

export type ListenerState = "off" | "starting" | "listening" | "error";
export type Transport = "usb" | "wifi";

export interface InvalidSetting {
  variable: string;
  value: string;
}

export interface ListenerStatus {
  state: ListenerState;
  port: number;
  address: string;
  errorDetail: string | null;
}

export type MdnsStatus =
  { state: "off" } | { state: "announced" } | { state: "error"; detail: string };

export interface LanAddress {
  address: string;
  interface: string;
  /** Wi-Fi or Ethernet: where tablets can reach the PC. */
  physical: boolean;
}

export interface WifiStatus {
  enabled: boolean;
  /** Set when PADBOARD_BIND or the test mode forces Wi-Fi on (true) or off (false). */
  forced: boolean | null;
  /** The test mode forces it (off), not PADBOARD_BIND. */
  forcedByTestMode: boolean;
  listener: ListenerStatus;
  lanAddresses: LanAddress[];
  fingerprint: string | null;
  tlsError: string | null;
  mdns: MdnsStatus;
}

export interface ServerStatus {
  name: string;
  usb: ListenerStatus;
  wifi: WifiStatus;
  invalidSettings: InvalidSetting[];
  /** This PC's version: tablets with an older app are offered it. */
  appVersion: string;
  /** Its tablet versionCode (null for a development version). */
  appVersionCode: number | null;
  /** A development version (X.Y.Z-dev): no published APK. */
  development: boolean;
  /** For a development version, the versionCode of its own tablet builds. */
  developmentCode: number | null;
}

export interface ClientInfo {
  id: number;
  deviceId: string;
  deviceName: string;
  transport: Transport;
  peer: string;
  connectedAtMs: number;
  /** The tablet is in kiosk mode. */
  kiosk: boolean;
  /** Its app's version and protocol, from `hello`. */
  tablet: TabletInfo;
  /** An update sent over this connection. */
  update: TabletUpdate | null;
}

/** Mirror of `server::TabletInfo`. */
export interface TabletInfo {
  protocolVersion: number;
  /** The protocols differ: the session only serves updates. */
  updateOnly: boolean;
  appVersion: string | null;
  appVersionCode: number | null;
  /** Installs updates without asking (kiosk mode). */
  silentInstall: boolean;
}

/** Mirror of `tablet_update::TabletUpdate`. */
export type TabletUpdate =
  | { state: "sending"; version: string; sent: number; total: number }
  | { state: "installing"; version: string }
  | { state: "failed"; version: string; outcome: UpdateOutcome };

/** A tablet whose Wi-Fi session went silent (the network may keep them apart). */
export interface DroppedTablet {
  deviceId: string;
  deviceName: string;
  atMs: number;
}

/** A paired tablet turned away: its app is too old for Wi-Fi updates. */
export interface OutdatedTablet {
  deviceId: string;
  deviceName: string;
  transport: Transport;
  atMs: number;
}

export type PairingView =
  | { state: "idle" }
  | { state: "active"; pin: string; expiresAtMs: number; attemptsLeft: number }
  | { state: "expired" }
  | { state: "locked" }
  | { state: "paired"; deviceName: string };

export interface PairedDevice {
  deviceId: string;
  name: string;
  pairedAtMs: number;
  lastSeenMs: number | null;
}

export type AdbState =
  | { state: "searching" }
  | { state: "missing"; chosen: string | null }
  | { state: "ready"; path: string; version: string | null }
  | { state: "error"; path: string; detail: string };

export type DeviceState =
  | { state: "unauthorized" }
  | { state: "noPermission" }
  | { state: "offline" }
  | { state: "checking" }
  | { state: "noPadboard" }
  | { state: "ready" }
  | { state: "failed"; detail: string }
  | { state: "other"; adbState: string };

/** Device owner read with `dpm list-owners` (adb.rs `KioskState`). */
export type KioskState =
  | { state: "unknown" }
  | { state: "off" }
  | { state: "on" }
  | { state: "otherOwner"; owner: string };

/** Why kiosk mode was refused (adb.rs `KioskError`). */
export type KioskError =
  | { code: "testMode" | "noAdb" | "busy" | "accounts" | "users" }
  | { code: "otherOwner"; owner: string }
  | { code: "failed"; detail: string };

/** Why a USB tablet could not be updated (adb.rs `UsbUpdateError`). */
export type UsbUpdateError =
  | {
      code:
        "testMode" | "noAdb" | "busy" | "noPadboard" | "notNewer" | "signatureMismatch" | "storage";
    }
  | { code: "apk"; error: ApkError }
  | { code: "failed"; detail: string };

/** Why an update could not be sent over Wi-Fi (tablet_update.rs `WifiUpdateError`). */
export type WifiUpdateError =
  | { code: "apk"; error: ApkError }
  | { code: "offer"; reason: "gone" | "notSilent" | "notNewer" }
  | { code: "tooLarge" };

/** Why the tablet APK is not available (updates/tablet.rs `ApkError`). */
export type ApkError =
  | { code: "noKey" }
  | { code: "notPublished"; version: string }
  | { code: "network" | "signature" | "storage"; detail: string };

/** An installed app's version (adb.rs `AppVersion`). */
export interface AppVersion {
  name: string | null;
  code: number;
}

export interface UsbDevice {
  serial: string;
  model: string | null;
  state: DeviceState;
  /** The installed Padboard's version. */
  app: AppVersion | null;
  kiosk: KioskState;
}

export interface UsbStatus {
  adb: AdbState;
  devices: UsbDevice[];
}

export type ActionErrorCode = "invalid" | "unsupported" | "unavailable" | "failed";

export type ActionOutcome =
  { status: "ok" } | { status: "error"; code: ActionErrorCode; detail: string };

export interface ActivityEntry {
  id: number;
  atMs: number;
  deviceName: string | null;
  buttonId: string;
  label: string | null;
  outcome: ActionOutcome;
}

export interface Snapshot {
  server: ServerStatus;
  clients: ClientInfo[];
  pairing: PairingView;
  pairedDevices: PairedDevice[];
  usb: UsbStatus;
  outdatedTablets: OutdatedTablet[];
  /** Paired tablets whose Wi-Fi session went silent and was dropped. */
  droppedWifi: DroppedTablet[];
  activity: ActivityEntry[];
}
