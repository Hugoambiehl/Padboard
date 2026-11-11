<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { GripVertical, Search } from "lucide-vue-next";
import IconPaths from "./IconPaths.vue";
import { invoke } from "@tauri-apps/api/core";
import { categoryColors } from "../../editor/colors";
import { loadIcons } from "../../icons/catalog";
import { iconToPaths } from "../../icons/paths";
import { addPreset, firstEmptyCell } from "../../editor/actions";
import { buttonAt, useSelection } from "../../editor/selection";
import { startDrag } from "../../editor/drag";
import { useEditor } from "../../composables/useEditor";
import type { Preset, PresetGroup } from "../../generated/types";

const emit = defineEmits<{ icons: [Record<string, string[]>] }>();
const { t } = useI18n();
const { editedProfile } = useEditor();
const { page, selected } = useSelection();

const presets = ref<Preset[]>([]);
const icons = ref<Record<string, string[]>>({});
const query = ref("");

onMounted(async () => {
  presets.value = await invoke<Preset[]>("library");
  const catalog = await loadIcons();
  icons.value = Object.fromEntries(
    presets.value.map((p) => [p.icon, catalog[p.icon] ? iconToPaths(catalog[p.icon]!) : []]),
  );
  emit("icons", icons.value);
});

const label = (p: Preset) => t(`library.presets.${p.id}.label`);
const description = (p: Preset) => t(`library.presets.${p.id}.desc`);

const groups = computed(() => {
  const q = query.value.trim().toLowerCase();
  const order = ["system", "media", "navigation", "obs"] as PresetGroup[];
  return order
    .map((group) => ({
      group,
      items: presets.value.filter(
        (p) =>
          p.group === group &&
          (!q || label(p).toLowerCase().includes(q) || description(p).toLowerCase().includes(q)),
      ),
    }))
    .filter((g) => g.items.length > 0);
});

/** Click (or Enter): into the selected empty cell, else the first free one. */
function place(preset: Preset) {
  const p = page.value;
  const grid = editedProfile.value?.grid;
  if (!p || !grid) return;
  const s = selected.value;
  const cell =
    s && s.pageId === p.id && !buttonAt(p, s.row, s.col)
      ? { row: s.row, col: s.col }
      : firstEmptyCell(p, grid);
  if (cell) void addPreset(preset, cell);
}

function pointerdown(event: PointerEvent, preset: Preset) {
  startDrag(event, { kind: "preset", preset }, (cell) => void addPreset(preset, cell));
}
</script>

<template>
  <div class="flex flex-col gap-[18px]">
    <div class="flex flex-col gap-1 px-1">
      <span class="font-display text-base font-semibold">{{ t("library.title") }}</span>
      <span class="text-[13px] text-muted">{{ t("library.subtitle") }}</span>
    </div>
    <label
      class="flex items-center gap-2 rounded-[10px] border border-line-2 bg-bg px-3 focus-within:border-accent"
    >
      <Search :size="16" class="shrink-0 text-muted" aria-hidden="true" />
      <span class="sr-only">{{ t("library.search") }}</span>
      <input
        v-model="query"
        :placeholder="t('library.searchPlaceholder')"
        class="min-w-0 flex-1 bg-transparent py-2.5 text-sm outline-none"
      />
    </label>
    <div class="flex flex-col gap-4">
      <div v-for="g in groups" :key="g.group" class="flex flex-col gap-1">
        <span class="px-1.5 pb-1 text-[11px] font-semibold tracking-[.08em] text-muted uppercase">
          {{ t(`library.groups.${g.group}`) }}
        </span>
        <button
          v-for="p in g.items"
          :key="p.id"
          type="button"
          class="flex cursor-grab items-center gap-2.5 rounded-[10px] border border-transparent px-2 py-[7px] text-left transition-colors hover:bg-surface-active active:scale-[.97]"
          :title="t('library.clickHint')"
          @pointerdown="pointerdown($event, p)"
          @click="place(p)"
        >
          <span
            class="flex size-[30px] shrink-0 items-center justify-center rounded-[9px]"
            :style="{ background: categoryColors[p.color].bg }"
          >
            <IconPaths
              :paths="icons[p.icon] ?? []"
              :size="16"
              :color="categoryColors[p.color].fg"
            />
          </span>
          <span class="flex min-w-0 flex-1 flex-col gap-px">
            <span class="text-sm font-medium">{{ label(p) }}</span>
            <span class="truncate text-xs text-muted">{{ description(p) }}</span>
          </span>
          <GripVertical :size="14" class="shrink-0 text-[#4A4E5A]" aria-hidden="true" />
        </button>
      </div>
      <p v-if="groups.length === 0 && presets.length > 0" class="px-1 text-sm text-muted">
        {{ t("library.noResult") }}
      </p>
    </div>
  </div>
</template>
