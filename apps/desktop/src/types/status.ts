// Hand-written mirror of apps/desktop/src-tauri/src/state.rs (UI state, not the
// tablet protocol). Keep both in sync.

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
  /** Set when PADBOARD_BIND forces Wi-Fi on (true) or off (false). */
  forced: boolean | null;
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
}

export interface ClientInfo {
  id: number;
  deviceId: string;
  deviceName: string;
  transport: Transport;
  peer: string;
  connectedAtMs: number;
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

export interface UsbDevice {
  serial: string;
  model: string | null;
  state: DeviceState;
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
  activity: ActivityEntry[];
}
