// Hand-written mirror of apps/desktop/src-tauri/src/updates/mod.rs
// (`UpdatesView`). Keep both in sync.

export interface UpdatePreferences {
  /** Look for a new version at launch and daily. */
  check: boolean;
  /** Release candidates too ("Versions de test"). */
  prereleases: boolean;
}

export type OffReason = "noKey" | "testMode";
export type UpdateError = "network" | "signature" | "install" | "storage";

export type UpdateStatus =
  | { state: "off"; reason: OffReason }
  | { state: "idle" }
  | { state: "checking" }
  /** `checkedAt`: milliseconds since the Unix epoch. */
  | { state: "upToDate"; checkedAt: number }
  /** `installable`: this PC downloads and checks the installer (Windows),
   * else the release page. `startsInstaller`: Padboard then starts it
   * itself (signed by the trusted publisher), else it shows it. */
  | { state: "available"; version: string; installable: boolean; startsInstaller: boolean }
  | { state: "downloading"; version: string; received: number; total: number | null }
  | { state: "installing"; version: string }
  | { state: "failed"; error: UpdateError }
  /** The installer of `version` was started, but this is still the older
   * version: cancelled, or blocked by an antivirus. */
  | { state: "notInstalled"; version: string }
  /** The checked installer is saved at `file` and shown in the Explorer:
   * the user quits Padboard and runs it. */
  | { state: "ready"; version: string; file: string };

export interface UpdatesView {
  current: string;
  preferences: UpdatePreferences;
  status: UpdateStatus;
}
