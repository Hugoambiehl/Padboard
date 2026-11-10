// Editor operations combining several pieces (i18n, icons, selection).
import { i18n } from "../i18n";
import { iconConfig } from "../icons/catalog";
import { useEditor } from "../composables/useEditor";
import { buttonAt, useSelection } from "./selection";
import { hotkeyLabel } from "./keys";
import type { GridSize, PageConfig, Preset } from "../generated/types";

/** First empty cell of a page, row by row. */
export function firstEmptyCell(
  page: PageConfig,
  grid: GridSize,
): { row: number; col: number } | null {
  for (let row = 0; row < grid.rows; row++) {
    for (let col = 0; col < grid.cols; col++) {
      if (!buttonAt(page, row, col)) return { row, col };
    }
  }
  return null;
}

/** Creates a button from a library action at a cell of the shown page, and selects it. */
export async function addPreset(preset: Preset, cell: { row: number; col: number }) {
  const { edit } = useEditor();
  const { page, select } = useSelection();
  const target = page.value;
  if (!target || buttonAt(target, cell.row, cell.col)) return;
  const t = i18n.global.t;
  const keys = preset.action.type === "hotkey" ? preset.action.params.keys : [];
  const outcome = await edit({
    type: "addButton",
    params: {
      pageId: target.id,
      row: cell.row,
      col: cell.col,
      label: t(`library.presets.${preset.id}.label`),
      subtitle: preset.group === "system" && keys.length > 0 ? hotkeyLabel(keys) : undefined,
      icon: (await iconConfig(preset.icon)) ?? undefined,
      color: preset.color,
      action: preset.action,
    },
  });
  if (outcome) select({ pageId: target.id, ...cell });
}
