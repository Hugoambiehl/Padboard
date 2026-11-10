<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import IconPaths from "./IconPaths.vue";
import { drag } from "../../editor/drag";
import { categoryColors } from "../../editor/colors";

/** The tile following the pointer while dragging (transform only). */
const props = defineProps<{ presetIcons: Record<string, string[]> }>();
const { t } = useI18n();

const view = computed(() => {
  const s = drag.source;
  if (!s) return null;
  if (s.kind === "button") {
    return { color: s.button.color, paths: s.button.icon?.paths ?? [], label: s.button.label };
  }
  return {
    color: s.preset.color,
    paths: props.presetIcons[s.preset.icon] ?? [],
    label: t(`library.presets.${s.preset.id}.label`),
  };
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="view"
      class="pointer-events-none fixed top-0 left-0 z-50 flex w-40 items-center gap-2.5 rounded-xl border border-accent/60 bg-surface-2/95 p-2 shadow-2xl shadow-black/60"
      :style="{ transform: `translate3d(${drag.x + 12}px, ${drag.y + 12}px, 0)` }"
    >
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-[9px]"
        :style="{ background: categoryColors[view.color].bg }"
      >
        <IconPaths :paths="view.paths" :size="17" :color="categoryColors[view.color].fg" />
      </span>
      <span class="truncate text-xs font-semibold">{{ view.label }}</span>
    </div>
  </Teleport>
</template>
