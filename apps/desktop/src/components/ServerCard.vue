<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Server, Usb, Wifi } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import CopyField from "./CopyField.vue";
import type { ListenerState, ServerStatus } from "../types/status";

const props = defineProps<{ server: ServerStatus }>();
const { t } = useI18n();

const dot: Record<ListenerState, string> = {
  off: "bg-[#3A3D47]",
  starting: "bg-cat-orange",
  listening: "bg-cat-teal",
  error: "bg-cat-red",
};
const usb = computed(() => props.server.usb);
const wifi = computed(() => props.server.wifi);
const adbCommand = computed(() => `adb reverse tcp:${usb.value.port} tcp:${usb.value.port}`);
</script>

<template>
  <BaseCard :title="t('server.title')" :icon="Server">
    <p
      v-for="s in server.invalidSettings"
      :key="s.variable"
      class="rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-sm text-[#F3D2BA]"
    >
      {{ t("server.invalidSetting", { variable: s.variable, value: s.value }) }}
    </p>

    <div class="flex flex-col gap-2">
      <h3 class="flex items-center gap-2 text-sm font-semibold">
        <Usb :size="16" class="text-cat-blue" aria-hidden="true" />
        {{ t("usb.title") }}
        <span class="ml-auto flex items-center gap-2 text-xs font-normal text-muted">
          <span class="size-2 rounded-full" :class="dot[usb.state]" />
          {{ t(`server.state.${usb.state}`) }}
        </span>
      </h3>
      <p
        v-if="usb.state === 'error'"
        class="rounded-xl border border-cat-red/40 bg-cat-red/10 p-3 text-sm text-[#FFC2C2]"
      >
        {{ t("server.bindError", { address: usb.address }) }}
        <span v-if="usb.errorDetail" class="mt-1 block font-mono text-xs opacity-80">
          {{ usb.errorDetail }}
        </span>
      </p>
      <p class="text-sm text-muted">{{ t("usb.hint", { address: "127.0.0.1" }) }}</p>
      <CopyField :value="adbCommand" />
    </div>

    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <h3 class="flex items-center gap-2 text-sm font-semibold">
        <Wifi :size="16" class="text-cat-violet" aria-hidden="true" />
        {{ t("lan.title") }}
        <span class="ml-auto flex items-center gap-2 text-xs font-normal text-muted">
          <span class="size-2 rounded-full" :class="dot[wifi.listener.state]" />
          {{ t(`server.state.${wifi.listener.state}`) }}
        </span>
      </h3>
      <p
        v-if="wifi.tlsError"
        class="rounded-xl border border-cat-red/40 bg-cat-red/10 p-3 text-sm text-[#FFC2C2]"
      >
        {{ t("lan.tlsError") }}
        <span class="mt-1 block font-mono text-xs opacity-80">{{ wifi.tlsError }}</span>
      </p>
      <p
        v-else-if="wifi.listener.state === 'error'"
        class="rounded-xl border border-cat-red/40 bg-cat-red/10 p-3 text-sm text-[#FFC2C2]"
      >
        {{ t("server.bindError", { address: wifi.listener.address }) }}
        <span v-if="wifi.listener.errorDetail" class="mt-1 block font-mono text-xs opacity-80">
          {{ wifi.listener.errorDetail }}
        </span>
      </p>
      <template v-if="wifi.enabled">
        <template v-if="wifi.lanAddresses.length > 0">
          <p class="text-sm text-muted">{{ t("lan.hint", { port: wifi.listener.port }) }}</p>
          <ul class="flex flex-wrap gap-2">
            <li
              v-for="ip in wifi.lanAddresses"
              :key="ip"
              class="rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1 font-mono text-sm"
            >
              {{ ip }}
            </li>
          </ul>
        </template>
        <p v-else class="text-sm text-muted">{{ t("lan.none") }}</p>
      </template>
      <template v-else>
        <p class="text-sm text-muted">{{ t("lan.disabled") }}</p>
        <CopyField value="PADBOARD_BIND=lan" />
      </template>
    </div>
  </BaseCard>
</template>
