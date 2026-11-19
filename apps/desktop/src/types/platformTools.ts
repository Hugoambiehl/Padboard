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
  /** Not the pinned archive; `reason` guessed from what was received. */
  | { code: "checksum"; reason: ChecksumMismatch; received: number; expected: number }
  | { code: "signature"; file: string; detail: string }
  | { code: "install"; detail: string };

/** `Mismatch`: a web page (portal, proxy), a cut transfer, another archive
 * (Google republished it), anything else. */
export type ChecksumMismatch = "html" | "truncated" | "changed" | "unexpected";
