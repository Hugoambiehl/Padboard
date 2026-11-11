import { describe, expect, it } from "vitest";
import fr from "../locales/fr.json";
import { kioskErrorMessage } from "./kiosk";

function text(key: string): unknown {
  return key
    .split(".")
    .reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], fr);
}

describe("kioskErrorMessage", () => {
  it("names the app that already owns the tablet", () => {
    expect(kioskErrorMessage({ code: "otherOwner", owner: "com.example.mdm" })).toEqual({
      key: "usbDevices.kiosk.error.otherOwner",
      params: { owner: "com.example.mdm" },
    });
  });

  it("explains the refusals Android gives", () => {
    for (const code of ["accounts", "users", "busy", "noAdb", "testMode"] as const) {
      expect(kioskErrorMessage({ code }).key).toBe(`usbDevices.kiosk.error.${code}`);
    }
  });

  it("keeps the short message of anything else", () => {
    expect(kioskErrorMessage({ code: "failed", detail: "device offline" }).params).toEqual({
      detail: "device offline",
    });
    expect(kioskErrorMessage("boom").params).toEqual({ detail: "boom" });
  });

  it("has a French text for every key", () => {
    for (const code of ["accounts", "users", "busy", "noAdb", "testMode", "otherOwner", "failed"]) {
      expect(typeof text(`usbDevices.kiosk.error.${code}`)).toBe("string");
    }
  });
});
