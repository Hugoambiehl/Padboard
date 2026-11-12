import type { Ref } from "vue";
import { describe, expect, it } from "vitest";
import { i18n, type AppLocale } from "../i18n";
import { keyLabel } from "./keys";
// The backend's key table (`keys!` in padboard-actions).
import keysSource from "../../../../crates/actions/src/keys.rs?raw";

/** Every key the backend accepts. */
function backendKeys(): string[] {
  const table = keysSource.slice(keysSource.indexOf("keys! {"));
  return [...table.matchAll(/^\s+([A-Z][A-Za-z0-9]+) =>/gm)].map((m) => m[1] ?? "");
}

// Composition mode (legacy: false): the global locale is a ref.
const setLocale = (locale: AppLocale) =>
  ((i18n.global.locale as unknown as Ref<AppLocale>).value = locale);

describe("keyLabel", () => {
  const keys = backendKeys();

  it("reads the backend's keys", () => {
    expect(keys.length).toBeGreaterThan(100);
    expect(keys).toContain("IntlBackslash");
  });

  for (const locale of ["fr", "en"] as const) {
    it(`names every key in ${locale}`, () => {
      setLocale(locale);
      // Letters, digits and F1…F24 are named after their code.
      const generic = /^(Key[A-Z]|Digit\d|Numpad\d|F\d+)$/;
      const unnamed = keys.filter((k) => !generic.test(k) && !i18n.global.te(`keys.${k}`, locale));
      expect(unnamed).toEqual([]);
    });
  }

  it("translates", () => {
    setLocale("fr");
    expect(keyLabel("ArrowDown")).toBe("Flèche bas");
    expect(keyLabel("KeyQ")).toBe("Q");
    expect(keyLabel("Numpad4")).toBe("Pavé 4");
  });
});
