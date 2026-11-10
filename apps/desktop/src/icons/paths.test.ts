import { describe, expect, it } from "vitest";
import { icons } from "lucide";
import { iconToPaths, type IconNode } from "./paths";

describe("iconToPaths", () => {
  it("keeps paths and converts every element kind", () => {
    const node: IconNode = [
      ["path", { d: "M12 19v3" }],
      ["circle", { cx: 12, cy: 12, r: 3 }],
      ["ellipse", { cx: 12, cy: 5, rx: 9, ry: 3 }],
      ["rect", { x: 2, y: 4, width: 20, height: 16 }],
      ["rect", { x: 9, y: 2, width: 6, height: 13, rx: 3 }],
      ["line", { x1: 2, y1: 2, x2: 22, y2: 22 }],
      ["polyline", { points: "22 12 18 12 15 21" }],
      ["polygon", { points: "12 2 2 7 12 12" }],
      ["unknown", {}],
    ];
    expect(iconToPaths(node)).toEqual([
      "M12 19v3",
      "M9 12a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z",
      "M3 5a9 3 0 1 0 18 0a9 3 0 1 0 -18 0z",
      "M2 4h20v16h-20z",
      "M12 2H12A3 3 0 0 1 15 5V12A3 3 0 0 1 12 15H12A3 3 0 0 1 9 12V5A3 3 0 0 1 12 2z",
      "M2 2L22 22",
      "M22 12L18 12L15 21",
      "M12 2L2 7L12 12z",
    ]);
  });

  it("converts every Lucide icon to bounded, non-empty paths", () => {
    for (const [name, node] of Object.entries(icons)) {
      const paths = iconToPaths(node as IconNode);
      expect(paths.length, name).toBeGreaterThan(0);
      expect(paths.length, name).toBeLessThanOrEqual(32);
      expect(paths.join("").length, name).toBeLessThanOrEqual(2048);
    }
  });
});
