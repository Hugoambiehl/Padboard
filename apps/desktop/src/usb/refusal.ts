// A connection refused on the USB port (usb_peer.rs): what the USB card says.
import type { UsbRefusal } from "../types/status";

/** How long a refusal stays in the USB card. */
export const REFUSAL_SHOWN_MS = 5 * 60_000;

/** i18n keys and parameters: what happened, then what to do. */
export function usbRefusalMessage(refusal: UsbRefusal): {
  key: string;
  params: Record<string, string>;
  hintKey: string;
} {
  if (refusal.unverifiable) {
    return {
      key: `usbDevices.refused.unverifiable.${refusal.unverifiable}`,
      params: {},
      hintKey:
        refusal.unverifiable === "peerNotFound"
          ? "usbDevices.refused.hint.retry"
          : refusal.unverifiable === "noAdbServer"
            ? "usbDevices.refused.hint.replug"
            : "usbDevices.refused.hint.ownAccount",
    };
  }
  if (refusal.program) {
    return {
      key: "usbDevices.refused.notAdb.program",
      params: { program: refusal.program, pid: String(refusal.pid ?? "?") },
      hintKey: "usbDevices.refused.hint.notAdb",
    };
  }
  return {
    key: "usbDevices.refused.notAdb.unknown",
    params: {},
    hintKey: "usbDevices.refused.hint.notAdb",
  };
}

/** The refusal to show at `now`, if recent enough. */
export function recentRefusal(refusal: UsbRefusal | null, now: number): UsbRefusal | null {
  return refusal && now - refusal.atMs < REFUSAL_SHOWN_MS ? refusal : null;
}
