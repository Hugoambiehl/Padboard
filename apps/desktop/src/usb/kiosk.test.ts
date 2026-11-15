import { describe, expect, it } from "vitest";
import fr from "../locales/fr.json";
import { kioskErrorMessage, RELEASE_ARM_MS, releaseClick } from "./kiosk";

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

describe("releaseClick", () => {
  it("arms on the first click and confirms on a later second one", () => {
    const first = releaseClick(null, 7, 1000);
    expect(first).toEqual({ arm: { id: 7, at: 1000 }, confirmed: false });
    expect(releaseClick(first.arm, 7, 1000 + RELEASE_ARM_MS)).toEqual({
      arm: null,
      confirmed: true,
    });
  });

  it("ignores a double-click: the second click comes too soon", () => {
    const first = releaseClick(null, 7, 1000);
    const second = releaseClick(first.arm, 7, 1250);
    expect(second).toEqual({ arm: { id: 7, at: 1000 }, confirmed: false });
    expect(releaseClick(second.arm, 7, 1000 + RELEASE_ARM_MS).confirmed).toBe(true);
  });

  it("arms another tablet afresh", () => {
    const first = releaseClick(null, 7, 1000);
    expect(releaseClick(first.arm, 8, 5000)).toEqual({
      arm: { id: 8, at: 5000 },
      confirmed: false,
    });
  });
});
