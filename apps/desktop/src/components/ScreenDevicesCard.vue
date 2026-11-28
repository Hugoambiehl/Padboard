<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { MonitorUp } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import { forgetScreenDevice } from "../commands";
import type { ScreenDevice } from "../types/status";

/** Devices allowed to start a second screen without asking. */
defineProps<{ devices: ScreenDevice[] }>();
const { t, d } = useI18n();
const error = ref<string | null>(null);

async function forget(deviceId: string) {
  error.value = null;
  try {
    await forgetScreenDevice(deviceId);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <BaseCard :title="t('screenDevices.title')" :icon="MonitorUp" data-screen-devices>
    <ul class="flex flex-col gap-2">
      <li
        v-for="device in devices"
        :key="device.deviceId"
        class="flex items-center gap-3 rounded-xl border border-line-2 bg-surface-2 px-3 py-2.5"
      >
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-sm font-medium">{{ device.name }}</span>
          <span class="text-xs text-muted">
            {{
              t("screenDevices.allowedAt", { date: d(new Date(device.allowedAtMs), "dateTime") })
            }}
          </span>
        </div>
        <button
          type="button"
          class="shrink-0 rounded-lg border border-line-2 bg-surface px-2.5 py-1.5 text-xs text-muted-2 transition-colors hover:bg-surface-active hover:text-fg"
          data-screen-forget
          @click="forget(device.deviceId)"
        >
          {{ t("screenDevices.forget") }}
        </button>
      </li>
    </ul>
    <p class="text-xs text-muted">{{ t("screenDevices.hint") }}</p>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
