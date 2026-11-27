import { describe, expect, it } from "vitest";
import { recentRefusal, REFUSAL_SHOWN_MS, usbRefusalMessage } from "./refusal";
import type { UsbRefusal } from "../types/status";

const base: UsbRefusal = { atMs: 1000, pid: null, program: null, unverifiable: null };

describe("usbRefusalMessage", () => {
  it("names the program that is not adb", () => {
    expect(usbRefusalMessage({ ...base, pid: 42, program: "/usr/bin/nc" })).toEqual({
      key: "usbDevices.refused.notAdb.program",
      params: { program: "/usr/bin/nc", pid: "42" },
      hintKey: "usbDevices.refused.hint.notAdb",
    });
  });

  it("says so when the program is hidden (another account)", () => {
    expect(usbRefusalMessage(base).key).toBe("usbDevices.refused.notAdb.unknown");
  });

  it("explains what cannot be checked, and what to do", () => {
    expect(usbRefusalMessage({ ...base, unverifiable: "noAdbServer" }).hintKey).toBe(
      "usbDevices.refused.hint.replug",
    );
    expect(usbRefusalMessage({ ...base, unverifiable: "peerNotFound" }).hintKey).toBe(
      "usbDevices.refused.hint.retry",
    );
    for (const why of ["ownersUnknown", "tableUnreadable"] as const) {
      const message = usbRefusalMessage({ ...base, unverifiable: why });
      expect(message.key).toBe(`usbDevices.refused.unverifiable.${why}`);
      expect(message.hintKey).toBe("usbDevices.refused.hint.ownAccount");
    }
  });
});

describe("recentRefusal", () => {
  it("shows a refusal for five minutes", () => {
    expect(recentRefusal(base, 1000 + REFUSAL_SHOWN_MS - 1)).toBe(base);
    expect(recentRefusal(base, 1000 + REFUSAL_SHOWN_MS)).toBeNull();
    expect(recentRefusal(null, 0)).toBeNull();
  });
});
