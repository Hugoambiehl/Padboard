<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Download, Lock, Tablet, TriangleAlert } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import { exitKiosk, updateWifiTablet } from "../commands";
import type { ClientInfo, OutdatedTablet, TabletUpdate } from "../types/status";
import { needsUpdate, wifiUpdateErrorMessage } from "../usb/update";

defineProps<{
  clients: ClientInfo[];
  /** Paired tablets turned away: too old to be updated over Wi-Fi. */
  outdated: OutdatedTablet[];
  /** This PC's version, offered to older tablets, and its tablet versionCode. */
  appVersion: string;
  appVersionCode: number | null;
}>();
const { t, d } = useI18n();

/** Tablet whose release from kiosk mode waits for a second click. */
const confirming = ref<number | null>(null);
const error = ref<string | null>(null);

/** Updates over Wi-Fi: the request being sent, and why it was refused. */
const requesting = ref<number | null>(null);
const updateError = ref<Record<number, string>>({});

async function update(id: number) {
  if (requesting.value !== null) return;
  requesting.value = id;
  updateError.value = Object.fromEntries(
    Object.entries(updateError.value).filter(([key]) => Number(key) !== id),
  );
  try {
    await updateWifiTablet(id);
  } catch (e) {
    const { key, params } = wifiUpdateErrorMessage(e);
    updateError.value = { ...updateError.value, [id]: t(key, params) };
  } finally {
    requesting.value = null;
  }
}

/** What the tablet's update looks like now. */
function progress(u: TabletUpdate): { text: string; tone: string } {
  switch (u.state) {
    case "sending":
      return {
        text: t("tabletUpdate.sending", {
          version: u.version,
          percent: u.total > 0 ? Math.floor((u.sent / u.total) * 100) : 0,
        }),
        tone: "text-muted-2",
      };
    case "installing":
      return {
        text: t("tabletUpdate.outcome.installing", { version: u.version }),
        tone: "text-cat-teal",
      };
    case "failed":
      return { text: t(`tabletUpdate.outcome.${u.outcome}`), tone: "text-cat-red" };
  }
  return { text: "", tone: "" };
}

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
              <template v-if="c.tablet.appVersion">
                · {{ t("tabletUpdate.version", { version: c.tablet.appVersion }) }}
              </template>
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
        <p v-if="c.tablet.updateOnly" class="text-xs text-cat-orange" :data-update-only="c.id">
          {{
            appVersionCode != null &&
            c.tablet.appVersionCode != null &&
            c.tablet.appVersionCode > appVersionCode
              ? t("tabletUpdate.pcOutdated")
              : t("tabletUpdate.updateOnly")
          }}
        </p>
        <template
          v-if="c.transport === 'wifi' && needsUpdate(c.tablet.appVersionCode, appVersionCode)"
        >
          <button
            v-if="c.tablet.silentInstall"
            type="button"
            class="flex items-center gap-1.5 self-start rounded-lg bg-accent px-2.5 py-1.5 text-xs font-medium text-[#0E0F12] transition-opacity hover:opacity-90 disabled:cursor-wait disabled:opacity-50"
            :disabled="
              requesting !== null ||
              c.update?.state === 'sending' ||
              c.update?.state === 'installing'
            "
            :title="t('tabletUpdate.hint')"
            :data-wifi-update="c.id"
            @click="update(c.id)"
          >
            <Download :size="13" aria-hidden="true" />
            {{ t("tabletUpdate.update", { version: appVersion }) }}
          </button>
          <p v-else class="text-xs text-muted-2">{{ t("tabletUpdate.useUsb") }}</p>
        </template>
        <p
          v-if="c.update"
          class="text-xs"
          :class="progress(c.update).tone"
          data-wifi-update-progress
        >
          {{ progress(c.update).text }}
        </p>
        <p v-if="updateError[c.id]" class="text-xs text-cat-red">{{ updateError[c.id] }}</p>
        <button
          v-if="c.kiosk && !c.tablet.updateOnly"
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
    <p
      v-for="o in outdated"
      :key="o.deviceId"
      class="flex items-start gap-2 rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-xs text-[#F3D2BA]"
      data-outdated
    >
      <TriangleAlert :size="14" class="mt-0.5 shrink-0" aria-hidden="true" />
      {{ t("tabletUpdate.outdated", { name: o.deviceName }) }}
    </p>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
