// Hand-made drag and drop with pointer events (HTML5 drag and drop behaves
// differently in WebKitGTK and WebView2). One drag at a time.
import { reactive } from "vue";
import type { ButtonConfig, Preset } from "../generated/types";

export type DragSource =
  { kind: "preset"; preset: Preset } | { kind: "button"; button: ButtonConfig };

/** Movement before a press becomes a drag, so clicks keep working. */
export const DRAG_THRESHOLD = 4;

export const drag = reactive({
  source: null as DragSource | null,
  /** Pointer position (viewport), for the ghost. */
  x: 0,
  y: 0,
  /** Hovered cell of the shown page. */
  target: null as { row: number; col: number } | null,
});

/** `"1,2"` (a `data-cell` attribute) → row 1, column 2. */
export function parseCell(value: string | null | undefined): { row: number; col: number } | null {
  const match = /^(\d+),(\d+)$/.exec(value ?? "");
  return match ? { row: Number(match[1]), col: Number(match[2]) } : null;
}

/** What dropping `source` on a cell does. */
export function dropEffect(source: DragSource, cellHasButton: boolean): "add" | "move" | "none" {
  if (source.kind === "button") return "move";
  return cellHasButton ? "none" : "add";
}

function cellUnder(x: number, y: number) {
  const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-cell]");
  return parseCell(element?.dataset.cell);
}

/**
 * Starts tracking a press. The drag begins after DRAG_THRESHOLD pixels;
 * `onDrop` receives the cell under the pointer when released. Escape cancels.
 */
export function startDrag(
  event: PointerEvent,
  source: DragSource,
  onDrop: (cell: { row: number; col: number }) => void,
) {
  if (event.button !== 0) return;
  const startX = event.clientX;
  const startY = event.clientY;
  let dragging = false;

  const move = (e: PointerEvent) => {
    if (!dragging) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) < DRAG_THRESHOLD) return;
      dragging = true;
      drag.source = source;
      document.body.classList.add("dragging");
    }
    drag.x = e.clientX;
    drag.y = e.clientY;
    drag.target = cellUnder(e.clientX, e.clientY);
  };
  const end = (drop: boolean) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key, true);
    const target = drag.target;
    if (dragging && drop && target) onDrop(target);
    drag.source = null;
    drag.target = null;
    document.body.classList.remove("dragging");
  };
  const up = () => end(true);
  const key = (e: KeyboardEvent) => {
    if (e.key === "Escape" && dragging) {
      e.preventDefault();
      e.stopPropagation();
      end(false);
    }
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key, true);
}
