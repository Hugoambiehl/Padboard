// Kiosk mode from the USB card: what a refusal says (codes from adb.rs).
import type { KioskError } from "../types/status";

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
