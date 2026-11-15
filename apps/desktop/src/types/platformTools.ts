// Hand-written mirror of apps/desktop/src-tauri/src/platform_tools/mod.rs
// (`PlatformToolsInfo`, `PlatformToolsError`). Keep both in sync.

export interface PlatformToolsInfo {
  /** "Installer adb" can be offered (Windows). */
  offered: boolean;
  /** The version Padboard downloads (pinned). */
  version: string;
  /** `adb` of the downloaded copy, when there is one. */
  installed: string | null;
}

export type PlatformToolsError =
  | { code: "unsupported" }
  | { code: "notAccepted" }
  /** Google no longer serves the pinned version. */
  | { code: "unavailable"; status: number }
  | { code: "network"; detail: string }
  | { code: "checksum" }
  | { code: "signature"; file: string; detail: string }
  | { code: "install"; detail: string };
