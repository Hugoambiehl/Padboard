// Hand-written mirror of apps/desktop/src-tauri/src/updates/mod.rs
// (`UpdatesView`). Keep both in sync.

export interface UpdatePreferences {
  /** Look for a new version at launch and daily. */
  check: boolean;
  /** Release candidates too ("Versions de test"). */
  prereleases: boolean;
}

export type OffReason = "noKey" | "testMode";
export type UpdateError = "network" | "signature" | "install";

export type UpdateStatus =
  | { state: "off"; reason: OffReason }
  | { state: "idle" }
  | { state: "checking" }
  /** `checkedAt`: milliseconds since the Unix epoch. */
  | { state: "upToDate"; checkedAt: number }
  /** `installable`: one click on this PC (Windows), else the release page. */
  | { state: "available"; version: string; installable: boolean }
  | { state: "downloading"; version: string; received: number; total: number | null }
  | { state: "installing"; version: string }
  | { state: "failed"; error: UpdateError };

export interface UpdatesView {
  current: string;
  preferences: UpdatePreferences;
  status: UpdateStatus;
}
