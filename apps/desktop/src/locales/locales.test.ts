import { describe, expect, it } from "vitest";
import fr from "./fr.json";
import en from "./en.json";

type Messages = { [key: string]: string | Messages };

/** Every leaf of `messages` as "a.b.c" → text. */
function leaves(messages: Messages, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(messages)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") out.set(path, value);
    else for (const [k, v] of leaves(value, path)) out.set(k, v);
  }
  return out;
}

const params = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
const plurals = (text: string) => text.split(" | ").length;

describe("locales", () => {
  const french = leaves(fr);
  const english = leaves(en);

  it("have the same keys", () => {
    expect([...english.keys()].sort()).toEqual([...french.keys()].sort());
  });

  it("use the same parameters and plural forms", () => {
    for (const [key, text] of french) {
      const other = english.get(key) ?? "";
      expect(params(other), key).toEqual(params(text));
      expect(plurals(other), key).toBe(plurals(text));
    }
  });

  it("have no empty text", () => {
    for (const [key, text] of [...french, ...english]) expect(text.trim(), key).not.toBe("");
  });
});
