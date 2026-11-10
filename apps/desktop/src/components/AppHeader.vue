<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { LayoutGrid } from "lucide-vue-next";

const props = defineProps<{ tablets: number }>();
const { t } = useI18n();
const connected = computed(() => props.tablets > 0);
</script>

<template>
  <header class="flex items-center justify-between gap-3 border-b border-line bg-surface px-5 py-3">
    <div class="flex items-center gap-3.5">
      <span class="flex size-8 items-center justify-center rounded-[10px] bg-accent">
        <LayoutGrid :size="17" class="text-bg" aria-hidden="true" />
      </span>
      <span class="font-display text-[17px] font-semibold">{{ t("app.name") }}</span>
    </div>
    <div
      class="flex items-center gap-2 rounded-full border px-3 py-1.5 text-[13px]"
      :class="
        connected
          ? 'border-cat-teal/35 bg-cat-teal/10 text-[#9FEBD9]'
          : 'border-line-2 bg-surface-2 text-muted'
      "
    >
      <span class="size-2 rounded-full" :class="connected ? 'bg-cat-teal' : 'bg-[#3A3D47]'" />
      {{ t("app.tablets", tablets) }}
    </div>
  </header>
</template>
