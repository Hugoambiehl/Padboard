<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useEditor } from "../composables/useEditor";

const { t } = useI18n();
const { activeProfile } = useEditor();
</script>

<template>
  <div class="flex min-h-0 flex-1">
    <aside class="w-[280px] shrink-0 border-r border-line bg-surface p-5" />
    <main class="flex min-w-0 flex-1 flex-col items-center gap-5 px-8 py-7">
      <p v-if="!activeProfile" class="text-sm text-muted">{{ t("app.loading") }}</p>
    </main>
    <aside class="w-[340px] shrink-0 border-l border-line bg-surface p-6" />
  </div>
</template>
