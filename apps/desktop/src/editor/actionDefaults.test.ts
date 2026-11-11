import { describe, expect, it } from "vitest";
import { ACTION_KINDS, defaultAction, hasActiveState } from "./actionDefaults";

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
