import { computed, ref, watch } from "vue";
import { useEditor } from "../composables/useEditor";
import type { ButtonConfig, PageConfig } from "../generated/types";

/** A cell of a page. Pages are referenced by id, never by position. */
export interface Cell {
  pageId: string;
  row: number;
  col: number;
}

const pageId = ref<string | null>(null);
const selected = ref<Cell | null>(null);
// Browser preview (src/dev/mock.ts): `?select=row,col` selects a cell of the first page.
const preselect = import.meta.env.DEV
  ? /^(\d+),(\d+)$/.exec(new URLSearchParams(location.search).get("select") ?? "")
  : null;
let watching = false;

/** The page shown in the editor and the selected cell, shared by all panels. */
export function useSelection() {
  const { editedProfile } = useEditor();

  const page = computed<PageConfig | null>(() => {
    const pages = editedProfile.value?.pages ?? [];
    return pages.find((p) => p.id === pageId.value) ?? pages[0] ?? null;
  });

  const selectedButton = computed<ButtonConfig | null>(() => {
    const cell = selected.value;
    if (!cell || !page.value || cell.pageId !== page.value.id) return null;
    return page.value.buttons.find((b) => b.row === cell.row && b.col === cell.col) ?? null;
  });

  if (!watching) {
    watching = true;
    // The page may disappear (deleted, other profile activated, undo): fall
    // back to the first page and drop a selection that no longer fits.
    watch(
      [editedProfile, page],
      ([profile, current]) => {
        if (current && pageId.value !== current.id) pageId.value = current.id;
        if (preselect && current && !selected.value && pageId.value === current.id) {
          selected.value = {
            pageId: current.id,
            row: Number(preselect[1]),
            col: Number(preselect[2]),
          };
        }
        const cell = selected.value;
        if (
          cell &&
          (!profile ||
            cell.pageId !== current?.id ||
            cell.row >= profile.grid.rows ||
            cell.col >= profile.grid.cols)
        ) {
          selected.value = null;
        }
      },
      { immediate: true },
    );
  }

  return {
    pageId,
    page,
    selected,
    selectedButton,
    showPage: (id: string) => {
      pageId.value = id;
      selected.value = null;
    },
    select: (cell: Cell | null) => (selected.value = cell),
  };
}

/** The button at a cell of a page. */
export function buttonAt(page: PageConfig, row: number, col: number): ButtonConfig | undefined {
  return page.buttons.find((b) => b.row === row && b.col === col);
}
