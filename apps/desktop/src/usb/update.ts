// Tablet updates from the PC: what a failure says (codes from adb.rs and
// updates/tablet.rs).
import type { ApkError, UsbUpdateError } from "../types/status";

export interface Text {
  key: string;
  params: Record<string, string>;
}

/** i18n key and parameters for an APK that could not be had. */
export function apkErrorMessage(error: ApkError): Text {
  switch (error.code) {
    case "noKey":
      return { key: "tabletUpdate.apk.noKey", params: {} };
    case "notPublished":
      return { key: "tabletUpdate.apk.notPublished", params: { version: error.version } };
    case "network":
    case "signature":
    case "storage":
      return { key: `tabletUpdate.apk.${error.code}`, params: { detail: error.detail } };
  }
}

/** i18n key and parameters for a refusal of `update_usb_tablet`. */
export function usbUpdateErrorMessage(error: unknown): Text {
  const e = error as Partial<UsbUpdateError> | null;
  switch (e?.code) {
    case "testMode":
    case "noAdb":
    case "busy":
    case "noPadboard":
    case "notNewer":
    case "signatureMismatch":
    case "storage":
      return { key: `tabletUpdate.usb.${e.code}`, params: {} };
    case "apk":
      return apkErrorMessage((e as { error: ApkError }).error);
    case "failed":
      return {
        key: "tabletUpdate.usb.failed",
        params: { detail: (e as { detail: string }).detail },
      };
    default:
      return { key: "tabletUpdate.usb.failed", params: { detail: String(error) } };
  }
}

/** The tablet's app is older than this PC's version (an update is offered). */
export function needsUpdate(appCode: number | null | undefined, pcCode: number | null): boolean {
  return appCode != null && pcCode != null && appCode < pcCode;
}
