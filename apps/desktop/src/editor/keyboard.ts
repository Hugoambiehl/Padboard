// Grid navigation with the keyboard (pure, unit-tested).
import type { GridSize } from "../generated/types";

export type Direction = "up" | "down" | "left" | "right";

const arrows: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

export function arrowDirection(key: string): Direction | null {
  return arrows[key] ?? null;
}

/** The neighbouring cell, or `null` at the edge of the grid. */
export function neighbor(
  row: number,
  col: number,
  direction: Direction,
  grid: GridSize,
): { row: number; col: number } | null {
  const next = {
    up: { row: row - 1, col },
    down: { row: row + 1, col },
    left: { row, col: col - 1 },
    right: { row, col: col + 1 },
  }[direction];
  const inside = next.row >= 0 && next.col >= 0 && next.row < grid.rows && next.col < grid.cols;
  return inside ? next : null;
}
