import { describe, expect, it } from "vitest";
import { HotkeyRecorder, normalize } from "./recorder";

const valid = new Set([
  "ControlLeft",
  "ShiftLeft",
  "AltLeft",
  "MetaLeft",
  "KeyT",
  "KeyC",
  "KeyA",
  "KeyB",
  "KeyD",
  "KeyE",
  "KeyF",
  "Escape",
]);

describe("HotkeyRecorder", () => {
  it("records a combination when every key is released", () => {
    const r = new HotkeyRecorder(valid);
    r.down("AltLeft");
    r.down("ControlLeft");
    r.down("KeyT");
    expect(r.up("KeyT")).toBeNull();
    expect(r.up("AltLeft")).toBeNull();
    expect(r.up("ControlLeft")).toEqual(["ControlLeft", "AltLeft", "KeyT"]);
  });

  it("ignores repeats and unknown keys", () => {
    const r = new HotkeyRecorder(valid);
    r.down("ControlLeft");
    r.down("ControlLeft");
    r.down("Fn");
    r.down("KeyC");
    r.up("KeyC");
    expect(r.up("ControlLeft")).toEqual(["ControlLeft", "KeyC"]);
  });

  it("records a single key, Escape included", () => {
    const r = new HotkeyRecorder(valid);
    r.down("Escape");
    expect(r.up("Escape")).toEqual(["Escape"]);
  });

  it("does nothing before a known key went down", () => {
    const r = new HotkeyRecorder(valid);
    expect(r.up("KeyT")).toBeNull();
  });
});

describe("normalize", () => {
  it("puts modifiers first in a fixed order and caps the length", () => {
    expect(normalize(["KeyT", "MetaLeft", "ShiftLeft", "ControlLeft"])).toEqual([
      "ControlLeft",
      "ShiftLeft",
      "MetaLeft",
      "KeyT",
    ]);
    expect(normalize(["KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyT"])).toHaveLength(6);
    expect(normalize(["KeyA", "KeyA"])).toEqual(["KeyA"]);
  });
});
