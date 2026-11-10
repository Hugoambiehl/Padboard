import { describe, expect, it } from "vitest";
import { arrowDirection, neighbor } from "./keyboard";

const grid = { cols: 5, rows: 3 };

describe("neighbor", () => {
  it("moves inside the grid", () => {
    expect(neighbor(1, 1, "up", grid)).toEqual({ row: 0, col: 1 });
    expect(neighbor(1, 1, "down", grid)).toEqual({ row: 2, col: 1 });
    expect(neighbor(1, 1, "left", grid)).toEqual({ row: 1, col: 0 });
    expect(neighbor(1, 1, "right", grid)).toEqual({ row: 1, col: 2 });
  });

  it("stops at the edges", () => {
    expect(neighbor(0, 0, "up", grid)).toBeNull();
    expect(neighbor(0, 0, "left", grid)).toBeNull();
    expect(neighbor(2, 4, "down", grid)).toBeNull();
    expect(neighbor(2, 4, "right", grid)).toBeNull();
  });

  it("maps arrow keys only", () => {
    expect(arrowDirection("ArrowLeft")).toBe("left");
    expect(arrowDirection("a")).toBeNull();
  });
});
