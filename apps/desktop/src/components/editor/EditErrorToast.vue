<script setup lang="ts">
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { AnimatePresence, motion } from "motion-v";
import { useEditor } from "../../composables/useEditor";

// A refused edit (validated by the desktop) shows briefly; nothing changed.
const { t, te } = useI18n();
const { lastError, clearError } = useEditor();
let timer: ReturnType<typeof setTimeout> | undefined;

watch(lastError, (error) => {
  clearTimeout(timer);
  if (error) timer = setTimeout(clearError, 4000);
});

const message = computed(() => {
  const code = lastError.value?.code;
  if (!code) return "";
  return te(`editErrors.${code}`) ? t(`editErrors.${code}`) : t("editErrors.unknown");
});
</script>

<template>
  <AnimatePresence>
    <motion.div
      v-if="lastError"
      role="alert"
      class="fixed bottom-5 left-1/2 z-40 max-w-md -translate-x-1/2 rounded-xl border border-cat-red/40 bg-[#2A1517] px-4 py-2.5 text-sm text-[#FFC2C2] shadow-2xl"
      :title="lastError.detail"
      :initial="{ opacity: 0, y: 8 }"
      :animate="{ opacity: 1, y: 0 }"
      :exit="{ opacity: 0, y: 8 }"
      :transition="{ duration: 0.15 }"
    >
      {{ message }}
    </motion.div>
  </AnimatePresence>
</template>
