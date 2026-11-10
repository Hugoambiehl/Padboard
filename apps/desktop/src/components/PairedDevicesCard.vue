<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { KeyRound } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import { revokeDevice } from "../commands";
import type { ClientInfo, PairedDevice } from "../types/status";

const props = defineProps<{ devices: PairedDevice[]; clients: ClientInfo[] }>();
const emit = defineEmits<{ pair: [] }>();
const { t, d } = useI18n();

/** Device whose revocation waits for a second click. */
const confirming = ref<string | null>(null);
const error = ref<string | null>(null);

function online(deviceId: string) {
  return props.clients.find((c) => c.deviceId === deviceId);
}

async function revoke(deviceId: string) {
  if (confirming.value !== deviceId) {
    confirming.value = deviceId;
    return;
  }
  confirming.value = null;
  error.value = null;
  try {
    await revokeDevice(deviceId);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <BaseCard :title="t('paired.title')" :icon="KeyRound">
    <p v-if="devices.length === 0" class="text-sm text-muted">
      {{ t("paired.empty") }}
      <button type="button" class="text-accent hover:underline" @click="emit('pair')">
        {{ t("paired.pairNow") }}
      </button>
    </p>
    <ul v-else class="flex flex-col gap-2">
      <li
        v-for="device in devices"
        :key="device.deviceId"
        class="flex items-center gap-3 rounded-xl border border-line-2 bg-surface-2 px-3 py-2.5"
      >
        <span
          class="size-2 shrink-0 rounded-full"
          :class="online(device.deviceId) ? 'bg-cat-teal' : 'bg-[#3A3D47]'"
        />
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-sm font-medium">{{ device.name }}</span>
          <span class="text-xs text-muted">
            <template v-if="online(device.deviceId)">
              {{
                t("paired.online", { via: t(`clients.via.${online(device.deviceId)!.transport}`) })
              }}
            </template>
            <template v-else-if="device.lastSeenMs">
              {{ t("paired.lastSeen", { date: d(new Date(device.lastSeenMs), "dateTime") }) }}
            </template>
            <template v-else>
              {{ t("paired.pairedAt", { date: d(new Date(device.pairedAtMs), "dateTime") }) }}
            </template>
          </span>
        </div>
        <button
          type="button"
          class="shrink-0 rounded-lg border px-2.5 py-1.5 text-xs transition-colors"
          :class="
            confirming === device.deviceId
              ? 'border-cat-red/60 bg-cat-red/15 text-[#FFC2C2] hover:bg-cat-red/25'
              : 'border-line-2 bg-surface text-muted-2 hover:bg-surface-active hover:text-fg'
          "
          @click="revoke(device.deviceId)"
          @blur="confirming = null"
        >
          {{ confirming === device.deviceId ? t("paired.confirmRevoke") : t("paired.revoke") }}
        </button>
      </li>
    </ul>
    <p v-if="devices.length > 0" class="text-xs text-muted">{{ t("paired.revokeHint") }}</p>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
