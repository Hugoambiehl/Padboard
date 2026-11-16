// Kiosk mode from the USB card: what a refusal says (codes from adb.rs).
import type { AppVersion, KioskError } from "../types/status";

/** i18n key and parameters for a refusal of `enable_kiosk`. */
export function kioskErrorMessage(error: unknown): { key: string; params: Record<string, string> } {
  const e = error as Partial<KioskError> | null;
  switch (e?.code) {
    case "testMode":
    case "noAdb":
    case "busy":
    case "accounts":
    case "users":
      return { key: `usbDevices.kiosk.error.${e.code}`, params: {} };
    case "otherOwner":
      return {
        key: "usbDevices.kiosk.error.otherOwner",
        params: { owner: (e as { owner: string }).owner },
      };
    case "failed":
      return {
        key: "usbDevices.kiosk.error.failed",
        params: { detail: (e as { detail: string }).detail },
      };
    default:
      return { key: "usbDevices.kiosk.error.failed", params: { detail: String(error) } };
  }
}

/**
 * "Libérer la tablette" takes two clicks, and the second one counts only
 * this long after the first: a double-click only arms the button.
 */
export const RELEASE_ARM_MS = 1000;

/** The tablet whose release button is armed, and since when. */
export type ReleaseArm = { id: number; at: number } | null;

/** A click on tablet `id`'s release button at `now`: arms it, is ignored (too soon), or confirms. */
export function releaseClick(
  arm: ReleaseArm,
  id: number,
  now: number,
): { arm: ReleaseArm; confirmed: boolean } {
  if (arm?.id !== id) return { arm: { id, at: now }, confirmed: false };
  if (now - arm.at < RELEASE_ARM_MS) return { arm, confirmed: false };
  return { arm: null, confirmed: true };
}

/**
 * First tablet `versionCode` whose exit gesture is in the bottom-left
 * corner (0.2.0-rc.2); 0.1.0 and 0.2.0-rc.1 use the top-right one.
 */
export const BOTTOM_LEFT_CORNER_FROM = 20002;

/**
 * i18n key of the "kiosk mode on" text, for the gesture of the version
 * installed on the tablet. A development version (`versionCode` ending in
 * 00) and an unknown one get the current gesture.
 */
export function kioskActiveKey(app: AppVersion | null): string {
  const old = app != null && app.code < BOTTOM_LEFT_CORNER_FROM && app.code % 100 !== 0;
  return old ? "usbDevices.kiosk.activeTopRight" : "usbDevices.kiosk.active";
}
