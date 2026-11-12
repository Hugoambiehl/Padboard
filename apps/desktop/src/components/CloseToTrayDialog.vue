<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { hideToTray, quitApp } from "../commands";

const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
const dialog = ref<HTMLDialogElement | null>(null);

onMounted(() => dialog.value?.showModal());

async function hide() {
  emit("close");
  await hideToTray();
}
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto w-[min(30rem,calc(100vw-2rem))] flex-col gap-4 rounded-2xl border border-line bg-surface p-6 text-fg open:flex backdrop:bg-black/60"
    aria-labelledby="close-to-tray-title"
    data-close-to-tray
    @cancel.prevent="emit('close')"
  >
    <div class="flex items-center gap-3">
      <img src="/favicon.svg" alt="" class="size-9" />
      <h2 id="close-to-tray-title" class="font-display text-lg font-semibold">
        {{ t("tray.closeTitle") }}
      </h2>
    </div>
    <p class="text-sm text-muted-2">{{ t("tray.closeText") }}</p>
    <div class="flex flex-wrap justify-end gap-2.5">
      <button
        type="button"
        class="rounded-[10px] border border-line-2 bg-surface-2 px-3.5 py-2 text-sm transition-colors hover:bg-surface-active"
        data-close-quit
        @click="quitApp()"
      >
        {{ t("tray.quit") }}
      </button>
      <button
        type="button"
        class="rounded-[10px] bg-accent px-3.5 py-2 text-sm font-medium text-bg transition-opacity hover:opacity-90"
        data-close-hide
        autofocus
        @click="hide"
      >
        {{ t("tray.hide") }}
      </button>
    </div>
  </dialog>
</template>
