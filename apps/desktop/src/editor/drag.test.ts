import { describe, expect, it } from "vitest";
import { dropEffect, parseCell, type DragSource } from "./drag";

const preset: DragSource = {
  kind: "preset",
  preset: {
    id: "copy",
    group: "system" as never,
    color: "blue" as never,
    icon: "Copy",
    action: { type: "hotkey", params: { keys: ["ControlLeft", "KeyC"] } },
  },
};

describe("drag and drop", () => {
  it("parses cell attributes", () => {
    expect(parseCell("1,2")).toEqual({ row: 1, col: 2 });
    expect(parseCell("12,0")).toEqual({ row: 12, col: 0 });
    expect(parseCell("1, 2")).toBeNull();
    expect(parseCell(undefined)).toBeNull();
  });

  it("drops actions on empty cells only and moves buttons anywhere", () => {
    expect(dropEffect(preset, false)).toBe("add");
    expect(dropEffect(preset, true)).toBe("none");
    const button: DragSource = { kind: "button", button: {} as never };
    expect(dropEffect(button, true)).toBe("move");
    expect(dropEffect(button, false)).toBe("move");
  });
});
