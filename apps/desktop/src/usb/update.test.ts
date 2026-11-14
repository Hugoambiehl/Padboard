import { describe, expect, it } from "vitest";
import fr from "../locales/fr.json";
import { devMismatch, needsUpdate, usbUpdateErrorMessage, wifiUpdateErrorMessage } from "./update";

function text(key: string): unknown {
  return key
    .split(".")
    .reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], fr);
}

describe("usbUpdateErrorMessage", () => {
  it("has a text for every refusal", () => {
    for (const code of [
      "testMode",
      "noAdb",
      "busy",
      "noPadboard",
      "notNewer",
      "signatureMismatch",
      "storage",
    ] as const) {
      const { key } = usbUpdateErrorMessage({ code });
      expect(text(key), key).toBeTypeOf("string");
    }
  });

  it("explains why the APK is not available", () => {
    expect(
      usbUpdateErrorMessage({ code: "apk", error: { code: "notPublished", version: "0.2.0" } }),
    ).toEqual({ key: "tabletUpdate.apk.notPublished", params: { version: "0.2.0" } });
    for (const code of ["network", "signature", "storage"] as const) {
      const message = usbUpdateErrorMessage({ code: "apk", error: { code, detail: "x" } });
      expect(text(message.key), message.key).toBeTypeOf("string");
      expect(message.params).toEqual({ detail: "x" });
    }
    expect(text(usbUpdateErrorMessage({ code: "apk", error: { code: "noKey" } }).key)).toBeTypeOf(
      "string",
    );
  });

  it("keeps the detail of anything else", () => {
    expect(usbUpdateErrorMessage({ code: "failed", detail: "device offline" }).params).toEqual({
      detail: "device offline",
    });
    expect(usbUpdateErrorMessage("boom").params).toEqual({ detail: "boom" });
  });
});

describe("devMismatch", () => {
  it("only for a development PC and an older tablet app", () => {
    expect(devMismatch(10099, true, 20000)).toBe(true);
    // A development tablet build of the same version: nothing to say.
    expect(devMismatch(20000, true, 20000)).toBe(false);
    expect(devMismatch(10099, false, null)).toBe(false);
    expect(devMismatch(null, true, 20000)).toBe(false);
  });
});

describe("needsUpdate", () => {
  it("only offers a newer version", () => {
    expect(needsUpdate(10099, 20099)).toBe(true);
    expect(needsUpdate(20099, 20099)).toBe(false);
    expect(needsUpdate(30099, 20099)).toBe(false);
    expect(needsUpdate(null, 20099)).toBe(false);
    expect(needsUpdate(10099, null)).toBe(false);
  });
});

describe("wifiUpdateErrorMessage", () => {
  it("has a text for every refusal", () => {
    for (const reason of ["gone", "notSilent", "notNewer"] as const) {
      const { key } = wifiUpdateErrorMessage({ code: "offer", reason });
      expect(text(key), key).toBeTypeOf("string");
    }
    expect(text(wifiUpdateErrorMessage({ code: "tooLarge" }).key)).toBeTypeOf("string");
    expect(
      wifiUpdateErrorMessage({ code: "apk", error: { code: "notPublished", version: "1.0.0" } })
        .params,
    ).toEqual({ version: "1.0.0" });
  });

  it("has a text for every outcome on the tablet", () => {
    for (const outcome of [
      "installing",
      "not_device_owner",
      "not_newer",
      "wrong_package",
      "bad_certificate",
      "bad_hash",
      "transfer_failed",
      "install_failed",
      "busy",
    ]) {
      expect(text(`tabletUpdate.outcome.${outcome}`), outcome).toBeTypeOf("string");
    }
  });
});
