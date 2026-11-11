import { describe, expect, it } from "vitest";
import { ACTION_KINDS, defaultAction, hasActiveState, subtitleAfterChange } from "./actionDefaults";
import type { Action } from "../generated/types";

describe("defaultAction", () => {
  it("gives each kind an action of that kind", () => {
    for (const kind of ACTION_KINDS) expect(defaultAction(kind).type).toBe(kind);
  });

  it("matches the JSON the desktop expects", () => {
    expect(defaultAction("lock")).toEqual({ type: "lock" });
    expect(defaultAction("multi")).toEqual({ type: "multi", params: { steps: [] } });
    expect(defaultAction("volume")).toEqual({ type: "volume", params: { command: "up" } });
  });
});

describe("hasActiveState", () => {
  it("is true for actions whose state shows on the tablet", () => {
    expect(hasActiveState(defaultAction("microphone"))).toBe(true);
    expect(hasActiveState(defaultAction("obs"))).toBe(true);
    expect(hasActiveState({ type: "volume", params: { command: "toggle_mute" as never } })).toBe(
      true,
    );
    expect(hasActiveState(defaultAction("volume"))).toBe(false);
    expect(hasActiveState(defaultAction("hotkey"))).toBe(false);
  });
});

describe("subtitleAfterChange", () => {
  const copy: Action = { type: "hotkey", params: { keys: ["ControlLeft", "KeyC"] } };
  const paste: Action = { type: "hotkey", params: { keys: ["ControlLeft", "KeyV"] } };
  const media = defaultAction("media");

  it("replaces the automatic text of the old action", () => {
    expect(subtitleAfterChange("Ctrl + C", copy, media)).toBe("");
    expect(subtitleAfterChange("Ctrl + C", copy, paste)).toBe("Ctrl + V");
    expect(subtitleAfterChange(undefined, media, copy)).toBe("Ctrl + C");
  });

  it("keeps a subtitle written by the user", () => {
    expect(subtitleAfterChange("Copier vite", copy, media)).toBeUndefined();
    expect(subtitleAfterChange("Médias", media, copy)).toBeUndefined();
  });

  it("changes nothing when the automatic text stays the same", () => {
    expect(subtitleAfterChange("", media, defaultAction("volume"))).toBeUndefined();
  });
});
