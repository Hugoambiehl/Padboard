<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Server, Usb, Wifi } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import CopyField from "./CopyField.vue";
import type { ServerStatus } from "../types/status";

const props = defineProps<{ server: ServerStatus }>();
const { t } = useI18n();

const stateDot = computed(
  () =>
    ({ starting: "bg-cat-orange", listening: "bg-cat-teal", error: "bg-cat-red" })[
      props.server.state
    ],
);
const adbCommand = computed(() => `adb reverse tcp:${props.server.port} tcp:${props.server.port}`);
</script>

<template>
  <BaseCard :title="t('server.title')" :icon="Server">
    <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
      <dt class="text-muted">{{ t("server.title") }}</dt>
      <dd class="flex items-center gap-2">
        <span class="size-2 rounded-full" :class="stateDot" />
        {{ t(`server.state.${server.state}`) }}
      </dd>
      <dt class="text-muted">{{ t("server.address") }}</dt>
      <dd class="font-mono">{{ server.address }}</dd>
      <dt class="text-muted">{{ t("server.mode") }}</dt>
      <dd>{{ t(`server.modes.${server.bindMode}`) }}</dd>
    </dl>

    <p
      v-if="server.state === 'error'"
      class="rounded-xl border border-cat-red/40 bg-cat-red/10 p-3 text-sm text-[#FFC2C2]"
    >
      {{ t("server.bindError", { address: server.address }) }}
      <span v-if="server.errorDetail" class="mt-1 block font-mono text-xs opacity-80">
        {{ server.errorDetail }}
      </span>
    </p>
    <p
      v-for="s in server.invalidSettings"
      :key="s.variable"
      class="rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-sm text-[#F3D2BA]"
    >
      {{ t("server.invalidSetting", { variable: s.variable, value: s.value }) }}
    </p>

    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <h3 class="flex items-center gap-2 text-sm font-semibold">
        <Usb :size="16" class="text-cat-blue" aria-hidden="true" />
        {{ t("usb.title") }}
      </h3>
      <p class="text-sm text-muted">{{ t("usb.hint", { address: "127.0.0.1" }) }}</p>
      <CopyField :value="adbCommand" />
    </div>

    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <h3 class="flex items-center gap-2 text-sm font-semibold">
        <Wifi :size="16" class="text-cat-violet" aria-hidden="true" />
        {{ t("lan.title") }}
      </h3>
      <template v-if="server.bindMode === 'lan'">
        <template v-if="server.lanAddresses.length > 0">
          <p class="text-sm text-muted">{{ t("lan.hint", { port: server.port }) }}</p>
          <ul class="flex flex-wrap gap-2">
            <li
              v-for="ip in server.lanAddresses"
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
