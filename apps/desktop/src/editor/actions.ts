// Editor operations combining several pieces (i18n, icons, selection).
import { i18n } from "../i18n";
import { iconConfig } from "../icons/catalog";
import { useEditor } from "../composables/useEditor";
import { buttonAt, useSelection } from "./selection";
import { autoSubtitle } from "./actionDefaults";
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
  if (preset.id === "folder") {
    // A folder comes with the button that opens it (one undo entry).
    const outcome = await edit({
      type: "addFolder",
      params: {
        pageId: target.id,
        row: cell.row,
        col: cell.col,
        name: t("folders.defaultName"),
        icon: (await iconConfig(preset.icon)) ?? undefined,
      },
    });
    if (outcome) select({ pageId: target.id, ...cell });
    return;
  }
  const outcome = await edit({
    type: "addButton",
    params: {
      pageId: target.id,
      row: cell.row,
      col: cell.col,
      label: t(`library.presets.${preset.id}.label`),
      subtitle: autoSubtitle(preset.action) || undefined,
      icon: (await iconConfig(preset.icon)) ?? undefined,
      color: preset.color,
      action: preset.action,
    },
  });
  if (outcome) select({ pageId: target.id, ...cell });
}
