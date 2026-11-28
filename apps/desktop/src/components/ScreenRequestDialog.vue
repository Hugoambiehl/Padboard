<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { MonitorUp } from "lucide-vue-next";
import { answerScreenRequest } from "../commands";
import type { ScreenRequest } from "../types/status";

/**
 * The PC's user is asked whether a device may show this PC's screen (the
 * first time; remembered only for a verified connection, SECURITY.md).
 * Refusing is the default (focus, Escape).
 */
const props = defineProps<{ request: ScreenRequest }>();
const { t } = useI18n();
const dialog = ref<HTMLDialogElement | null>(null);
const sent = ref(false);

onMounted(() => dialog.value?.showModal());

function answer(allow: boolean) {
  if (sent.value) return;
  sent.value = true;
  void answerScreenRequest(props.request.id, allow);
}
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto w-[min(32rem,calc(100vw-2rem))] flex-col gap-4 rounded-2xl border border-line bg-surface p-6 text-fg open:flex backdrop:bg-black/60"
    aria-labelledby="screen-request-title"
    data-screen-request
    @cancel.prevent="answer(false)"
  >
    <div class="flex items-center gap-3">
      <span class="grid size-9 place-items-center rounded-[10px] bg-cat-violet/15 text-cat-violet">
        <MonitorUp class="size-5" />
      </span>
      <h2 id="screen-request-title" class="font-display text-lg font-semibold">
        {{ t("screenRequest.title") }}
      </h2>
    </div>
    <p class="text-sm text-fg">
      {{
        t("screenRequest.text", {
          device: request.deviceName,
          via: t(`clients.via.${request.transport}`),
        })
      }}
    </p>
    <p class="text-sm text-muted-2">
      {{ request.remembered ? t("screenRequest.remembered") : t("screenRequest.notRemembered") }}
    </p>
    <div class="flex flex-wrap justify-end gap-2.5">
      <button
        type="button"
        class="rounded-[10px] bg-accent px-3.5 py-2 text-sm font-medium text-bg transition-opacity hover:opacity-90 disabled:opacity-50"
        data-screen-allow
        :disabled="sent"
        @click="answer(true)"
      >
        {{ t("screenRequest.allow") }}
      </button>
      <button
        type="button"
        class="order-first rounded-[10px] border border-line-2 bg-surface-2 px-3.5 py-2 text-sm transition-colors hover:bg-surface-active disabled:opacity-50"
        data-screen-refuse
        autofocus
        :disabled="sent"
        @click="answer(false)"
      >
        {{ t("screenRequest.refuse") }}
      </button>
    </div>
  </dialog>
</template>
