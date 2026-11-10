<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Plus } from "lucide-vue-next";
import ButtonFace from "./ButtonFace.vue";
import { useEditor } from "../../composables/useEditor";
import { buttonAt, useSelection } from "../../editor/selection";
import { arrowDirection, neighbor } from "../../editor/keyboard";

/** Cell highlighted as a drop target while dragging (see the library). */
defineProps<{ dropTarget?: { row: number; col: number } | null }>();
const emit = defineEmits<{ pointerdownButton: [event: PointerEvent, buttonId: string] }>();

const { t } = useI18n();
const { activeProfile, edit } = useEditor();
const { page, selected, select } = useSelection();

const grid = computed(() => activeProfile.value?.grid ?? { cols: 5, rows: 3 });
const cells = computed(() => {
  const p = page.value;
  if (!p) return [];
  const out = [];
  for (let row = 0; row < grid.value.rows; row++) {
    for (let col = 0; col < grid.value.cols; col++) {
      out.push({ row, col, button: buttonAt(p, row, col) });
    }
  }
  return out;
});

function isSelected(row: number, col: number) {
  const s = selected.value;
  return !!s && s.pageId === page.value?.id && s.row === row && s.col === col;
}

function pick(row: number, col: number) {
  if (page.value) select({ pageId: page.value.id, row, col });
}

// Arrows move the selection; Alt+arrows move the selected button (swapping
// with a neighbour); Delete removes it.
async function onKeydown(event: KeyboardEvent) {
  const p = page.value;
  if (!p) return;
  const cell = selected.value?.pageId === p.id ? selected.value : null;
  const direction = arrowDirection(event.key);
  if (direction) {
    event.preventDefault();
    if (!cell) return pick(0, 0);
    const next = neighbor(cell.row, cell.col, direction, grid.value);
    if (!next) return;
    const button = buttonAt(p, cell.row, cell.col);
    if (event.altKey && button) {
      const ok = await edit({
        type: "moveButton",
        params: { buttonId: button.id, pageId: p.id, row: next.row, col: next.col },
      });
      if (!ok) return;
    }
    pick(next.row, next.col);
  } else if ((event.key === "Delete" || event.key === "Backspace") && cell) {
    const button = buttonAt(p, cell.row, cell.col);
    if (button) {
      event.preventDefault();
      await edit({ type: "deleteButton", params: { buttonId: button.id } });
    }
  }
}
</script>

<template>
  <div
    class="w-full max-w-[820px] rounded-[30px] border border-line bg-[#08090B] p-[18px] outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
    tabindex="0"
    role="grid"
    :aria-label="t('grid.label')"
    @keydown="onKeydown"
  >
    <div
      class="grid gap-2.5 rounded-2xl bg-bg p-4"
      :style="{ gridTemplateColumns: `repeat(${grid.cols}, minmax(0, 1fr))` }"
    >
      <button
        v-for="c in cells"
        :key="`${c.row}-${c.col}`"
        type="button"
        tabindex="-1"
        :data-cell="`${c.row},${c.col}`"
        :aria-label="c.button?.label || t('grid.emptyCell', { row: c.row + 1, col: c.col + 1 })"
        :aria-selected="isSelected(c.row, c.col)"
        class="flex aspect-[1.1] min-w-0 flex-col rounded-xl border-2 p-2.5 text-left transition-[transform,border-color] duration-150 active:scale-95"
        :class="[
          c.button
            ? 'items-start justify-between bg-surface-2'
            : 'items-center justify-center gap-1.5 border-dashed bg-accent/[.06] text-[11px] text-muted-2',
          isSelected(c.row, c.col)
            ? 'border-accent'
            : dropTarget && dropTarget.row === c.row && dropTarget.col === c.col
              ? 'border-accent/70 bg-accent/15'
              : c.button
                ? 'border-line-2'
                : 'border-[#34374A]',
        ]"
        @click="pick(c.row, c.col)"
        @pointerdown="c.button && emit('pointerdownButton', $event, c.button.id)"
      >
        <ButtonFace v-if="c.button" :button="c.button" compact />
        <template v-else>
          <Plus :size="18" class="text-accent" aria-hidden="true" />
          <span v-if="dropTarget && dropTarget.row === c.row && dropTarget.col === c.col">
            {{ t("grid.dropHere") }}
          </span>
        </template>
      </button>
    </div>
  </div>
</template>
