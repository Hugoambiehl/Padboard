<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Server, ShieldCheck, Usb, Wifi } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import ToggleSwitch from "./ToggleSwitch.vue";
import { setWifiEnabled } from "../commands";
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
const physical = computed(() => wifi.value.lanAddresses.filter((a) => a.physical));
const others = computed(() => wifi.value.lanAddresses.filter((a) => !a.physical));
const forcedKey = computed(() =>
  wifi.value.forcedByTestMode
    ? "lan.forcedByTest"
    : wifi.value.forced
      ? "lan.forcedOn"
      : "lan.forcedOff",
);
const toggleLocked = computed(() => wifi.value.forced !== null || wifi.value.tlsError !== null);

const busy = ref(false);
const toggleError = ref<string | null>(null);
async function toggleWifi(enabled: boolean) {
  busy.value = true;
  toggleError.value = null;
  try {
    await setWifiEnabled(enabled);
  } catch (error) {
    toggleError.value = String(error);
  } finally {
    busy.value = false;
  }
}
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

    <!-- USB -->
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
      <p v-else class="text-sm text-muted">{{ t("usb.hint") }}</p>
    </div>

    <!-- Wi-Fi -->
    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <h3 class="flex items-center gap-2 text-sm font-semibold">
        <Wifi :size="16" class="text-cat-violet" aria-hidden="true" />
        {{ t("lan.title") }}
        <span class="ml-auto flex items-center gap-3 text-xs font-normal text-muted">
          <span class="flex items-center gap-2">
            <span class="size-2 rounded-full" :class="dot[wifi.listener.state]" />
            {{ t(`server.state.${wifi.listener.state}`) }}
          </span>
          <ToggleSwitch
            :checked="wifi.enabled"
            :disabled="toggleLocked || busy"
            :label="t('lan.toggle')"
            @toggle="toggleWifi"
          />
        </span>
      </h3>

      <p v-if="wifi.forced !== null" class="text-xs text-muted">
        {{ t(forcedKey) }}
      </p>
      <p v-if="toggleError" class="text-sm text-cat-red">{{ toggleError }}</p>
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
        <p class="text-sm text-muted">{{ t("lan.hint") }}</p>
        <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
          <dt class="text-muted">{{ t("lan.addresses") }}</dt>
          <dd class="flex flex-col gap-1">
            <span v-for="a in physical" :key="a.address" class="font-mono">
              {{ a.address }}:{{ wifi.listener.port }}
              <span class="font-sans text-xs text-muted">({{ a.interface }})</span>
            </span>
            <span v-if="physical.length === 0" class="text-muted">{{ t("lan.none") }}</span>
            <details v-if="others.length > 0" class="text-xs text-muted">
              <summary class="cursor-pointer select-none">
                {{ t("lan.otherInterfaces", others.length) }}
              </summary>
              <span v-for="a in others" :key="a.address" class="block font-mono">
                {{ a.address }} <span class="font-sans">({{ a.interface }})</span>
              </span>
            </details>
          </dd>
          <dt class="text-muted">{{ t("lan.discovery") }}</dt>
          <dd :title="wifi.mdns.state === 'error' ? wifi.mdns.detail : undefined">
            {{ t(`lan.mdns.${wifi.mdns.state}`) }}
          </dd>
        </dl>
      </template>
      <p v-else-if="!wifi.forcedByTestMode" class="text-sm text-muted">{{ t("lan.disabled") }}</p>

      <p
        v-if="wifi.fingerprint"
        class="flex items-center gap-2 text-xs text-muted"
        :title="t('lan.fingerprintHint')"
      >
        <ShieldCheck :size="14" aria-hidden="true" />
        {{ t("lan.fingerprint") }}
        <code class="font-mono text-muted-2">{{ wifi.fingerprint }}</code>
      </p>
    </div>
  </BaseCard>
</template>
