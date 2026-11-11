<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Lock, Tablet } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import { exitKiosk } from "../commands";
import type { ClientInfo } from "../types/status";

defineProps<{ clients: ClientInfo[] }>();
const { t, d } = useI18n();

/** Tablet whose release from kiosk mode waits for a second click. */
const confirming = ref<number | null>(null);
const error = ref<string | null>(null);

async function release(id: number) {
  if (confirming.value !== id) {
    confirming.value = id;
    return;
  }
  confirming.value = null;
  error.value = null;
  try {
    await exitKiosk(id);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <BaseCard :title="t('clients.title')" :icon="Tablet">
    <p v-if="clients.length === 0" class="text-sm text-muted">{{ t("clients.empty") }}</p>
    <ul v-else class="flex flex-col gap-2">
      <li
        v-for="c in clients"
        :key="c.id"
        class="flex flex-col gap-2 rounded-xl border border-line-2 bg-surface-2 px-3 py-2.5"
      >
        <div class="flex items-center gap-3">
          <span class="size-2 shrink-0 rounded-full bg-cat-teal" />
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-sm font-medium">{{ c.deviceName }}</span>
            <span class="text-xs text-muted">
              {{ t("clients.since", { time: d(new Date(c.connectedAtMs), "time") }) }}
            </span>
          </div>
          <span
            v-if="c.kiosk"
            class="flex items-center gap-1 rounded-full bg-accent/15 px-2.5 py-1 text-xs text-accent"
            :title="t('clients.kioskHint')"
          >
            <Lock :size="12" aria-hidden="true" />{{ t("clients.kiosk") }}
          </span>
          <span class="rounded-full bg-surface-active px-2.5 py-1 text-xs text-muted-2">
            {{ t(`clients.via.${c.transport}`) }}
          </span>
        </div>
        <button
          v-if="c.kiosk"
          type="button"
          class="self-end rounded-lg border px-2.5 py-1.5 text-xs transition-colors"
          :class="
            confirming === c.id
              ? 'border-cat-red/60 bg-cat-red/15 text-[#FFC2C2] hover:bg-cat-red/25'
              : 'border-line-2 bg-surface text-muted-2 hover:bg-surface-active hover:text-fg'
          "
          @click="release(c.id)"
          @blur="confirming = null"
        >
          {{ confirming === c.id ? t("clients.confirmExitKiosk") : t("clients.exitKiosk") }}
        </button>
      </li>
    </ul>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
