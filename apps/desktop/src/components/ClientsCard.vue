<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Tablet } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import type { ClientInfo } from "../types/status";

defineProps<{ clients: ClientInfo[] }>();
const { t, d } = useI18n();

/** Through `adb reverse`, a USB tablet reaches the server from the loopback address. */
function via(peer: string) {
  return peer.startsWith("127.") || peer.startsWith("[::1]")
    ? t("clients.via.usb")
    : t("clients.via.wifi");
}
</script>

<template>
  <BaseCard :title="t('clients.title')" :icon="Tablet">
    <p v-if="clients.length === 0" class="text-sm text-muted">{{ t("clients.empty") }}</p>
    <ul v-else class="flex flex-col gap-2">
      <li
        v-for="c in clients"
        :key="c.id"
        class="flex items-center gap-3 rounded-xl border border-line-2 bg-surface-2 px-3 py-2.5"
      >
        <span class="size-2 shrink-0 rounded-full bg-cat-teal" />
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-sm font-medium">{{ c.deviceName }}</span>
          <span class="text-xs text-muted">
            {{ t("clients.since", { time: d(new Date(c.connectedAtMs), "time") }) }}
          </span>
        </div>
        <span class="rounded-full bg-surface-active px-2.5 py-1 text-xs text-muted-2">
          {{ via(c.peer) }}
        </span>
      </li>
    </ul>
  </BaseCard>
</template>
