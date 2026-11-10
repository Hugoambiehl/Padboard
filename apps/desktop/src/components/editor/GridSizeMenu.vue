<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { onClickOutside } from "@vueuse/core";
import { AnimatePresence, motion } from "motion-v";
import { ChevronDown, Minus, Plus } from "lucide-vue-next";
import { useEditor } from "../../composables/useEditor";

const MAX_COLS = 8;
const MAX_ROWS = 6;

const { t } = useI18n();
const { activeProfile, edit, lastError } = useEditor();
const open = ref(false);
const root = ref<HTMLElement | null>(null);
const refused = ref(false);
onClickOutside(root, () => (open.value = false));

const grid = computed(() => activeProfile.value?.grid ?? { cols: 5, rows: 3 });

async function resize(cols: number, rows: number) {
  if (!activeProfile.value) return;
  const ok = await edit({
    type: "setGrid",
    params: { profileId: activeProfile.value.id, cols, rows },
  });
  refused.value = !ok && lastError.value?.code === "outsideGrid";
}
</script>

<template>
  <div ref="root" class="relative">
    <button
      type="button"
      class="flex items-center gap-2 rounded-[9px] border border-line-2 px-3 py-[7px] text-[13px] text-muted-2 hover:bg-surface-active"
      :aria-expanded="open"
      @click="
        open = !open;
        refused = false;
      "
    >
      {{ t("grid.size", { cols: grid.cols, rows: grid.rows }) }}
      <ChevronDown :size="14" class="text-muted" aria-hidden="true" />
    </button>
    <AnimatePresence>
      <motion.div
        v-if="open"
        class="absolute top-full right-0 z-20 mt-2 flex w-64 flex-col gap-3 rounded-xl border border-line-2 bg-surface-2 p-4 shadow-2xl shadow-black/50"
        :initial="{ opacity: 0, y: -4 }"
        :animate="{ opacity: 1, y: 0 }"
        :exit="{ opacity: 0, y: -4 }"
        :transition="{ duration: 0.12 }"
      >
        <div
          v-for="axis in ['cols', 'rows'] as const"
          :key="axis"
          class="flex items-center justify-between text-sm"
        >
          <span class="text-muted-2">{{ t(`grid.${axis}`) }}</span>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="stepper"
              :aria-label="t(`grid.less.${axis}`)"
              :disabled="grid[axis] <= 1"
              @click="
                resize(
                  axis === 'cols' ? grid.cols - 1 : grid.cols,
                  axis === 'rows' ? grid.rows - 1 : grid.rows,
                )
              "
            >
              <Minus :size="14" aria-hidden="true" />
            </button>
            <span class="w-5 text-center font-semibold tabular-nums">{{ grid[axis] }}</span>
            <button
              type="button"
              class="stepper"
              :aria-label="t(`grid.more.${axis}`)"
              :disabled="grid[axis] >= (axis === 'cols' ? MAX_COLS : MAX_ROWS)"
              @click="
                resize(
                  axis === 'cols' ? grid.cols + 1 : grid.cols,
                  axis === 'rows' ? grid.rows + 1 : grid.rows,
                )
              "
            >
              <Plus :size="14" aria-hidden="true" />
            </button>
          </div>
        </div>
        <p v-if="refused" class="text-xs text-[#F3D2BA]">{{ t("grid.wouldCut") }}</p>
      </motion.div>
    </AnimatePresence>
  </div>
</template>

<style scoped>
@reference "../../style.css";
.stepper {
  @apply flex size-7 items-center justify-center rounded-lg border border-line-2 hover:bg-surface-active disabled:pointer-events-none disabled:opacity-35;
}
</style>
