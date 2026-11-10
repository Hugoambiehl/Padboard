// Hand-written mirror of apps/desktop/src-tauri/src/state.rs (UI state, not the
// tablet protocol). Keep both in sync.

export type ServerState = "starting" | "listening" | "error";
export type BindMode = "loopback" | "lan";

export interface InvalidSetting {
  variable: string;
  value: string;
}

export interface ServerStatus {
  state: ServerState;
  bindMode: BindMode;
  port: number;
  address: string;
  lanAddresses: string[];
  errorDetail: string | null;
  invalidSettings: InvalidSetting[];
}

export interface ClientInfo {
  id: number;
  deviceName: string;
  peer: string;
  connectedAtMs: number;
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
  activity: ActivityEntry[];
}
