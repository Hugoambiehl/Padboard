<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Copy } from "lucide-vue-next";

const props = defineProps<{ value: string }>();
const { t } = useI18n();
const copied = ref(false);

async function copy() {
  try {
    await navigator.clipboard.writeText(props.value);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch (error) {
    console.warn("clipboard unavailable", error);
  }
}
</script>

<template>
  <div class="flex items-center gap-2 rounded-xl border border-line-2 bg-bg py-1 pr-1 pl-3">
    <code class="min-w-0 flex-1 truncate font-mono text-sm text-muted-2 select-all">{{
      value
    }}</code>
    <button
      type="button"
      class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs text-fg transition-colors hover:bg-surface-active"
      @click="copy"
    >
      <component :is="copied ? Check : Copy" :size="14" aria-hidden="true" />
      {{ copied ? t("server.copied") : t("server.copy") }}
    </button>
  </div>
</template>
