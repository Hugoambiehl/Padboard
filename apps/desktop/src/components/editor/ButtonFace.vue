<script setup lang="ts">
import { computed } from "vue";
import IconPaths from "./IconPaths.vue";
import { categoryColors } from "../../editor/colors";
import type { ButtonConfig } from "../../generated/types";

/** Icon tile and label of a button, as on the tablet (the label's initial without icon). */
const props = defineProps<{ button: ButtonConfig; compact?: boolean }>();
const colors = computed(() => categoryColors[props.button.color]);
const initial = computed(() => props.button.label.trim().charAt(0).toUpperCase());
</script>

<template>
  <span
    class="flex size-8 shrink-0 items-center justify-center rounded-[9px] font-display text-base font-semibold"
    :style="{ background: colors.bg, color: colors.fg }"
  >
    <IconPaths v-if="button.icon" :paths="button.icon.paths" :size="17" :color="colors.fg" />
    <template v-else>{{ initial }}</template>
  </span>
  <span class="flex w-full min-w-0 flex-col">
    <span class="truncate text-xs font-semibold">{{ button.label }}</span>
    <span v-if="!compact && button.subtitle" class="truncate text-[11px] text-muted">
      {{ button.subtitle }}
    </span>
  </span>
</template>
