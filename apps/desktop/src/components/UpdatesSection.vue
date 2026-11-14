<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Download, ExternalLink, RefreshCw } from "lucide-vue-next";
import ToggleSwitch from "./ToggleSwitch.vue";
import { checkForUpdates, installUpdate, openReleasePage, setUpdatePreferences } from "../commands";
import { useUpdates } from "../composables/useUpdates";
import type { UpdatePreferences } from "../types/updates";

const { t, locale } = useI18n();
const { updates } = useUpdates();
const error = ref<string | null>(null);

const status = computed(() => updates.value?.status ?? null);
const off = computed(() => status.value?.state === "off");
const busy = computed(() =>
  ["checking", "downloading", "installing"].includes(status.value?.state ?? ""),
);

/** The status line. */
const line = computed(() => {
  const s = status.value;
  if (!s) return "";
  switch (s.state) {
    case "off":
      return t(`updates.off.${s.reason}`);
    case "idle":
      return t("updates.idle");
    case "checking":
      return t("updates.checking");
    case "upToDate":
      return t("updates.upToDate", {
        time: new Date(s.checkedAt).toLocaleString(locale.value, {
          dateStyle: "short",
          timeStyle: "short",
        }),
      });
    case "available":
      return t("updates.available", { version: s.version });
    case "downloading":
      return s.total
        ? t("updates.downloadingPercent", {
            version: s.version,
            percent: Math.floor((s.received / s.total) * 100),
          })
        : t("updates.downloading", { version: s.version });
    case "installing":
      return t("updates.installing", { version: s.version });
    case "failed":
      return t(`updates.error.${s.error}`);
  }
  return "";
});

async function run(command: () => Promise<void>) {
  error.value = null;
  try {
    await command();
  } catch (e) {
    error.value = String(e);
  }
}

function save(change: Partial<UpdatePreferences>) {
  const current = updates.value;
  if (!current) return;
  const preferences = { ...current.preferences, ...change };
  void run(async () => {
    await setUpdatePreferences(preferences);
    if (updates.value) updates.value = { ...updates.value, preferences };
  });
}
</script>

<template>
  <div v-if="updates" class="flex flex-col gap-3 border-t border-line pt-4" data-updates>
    <h3 class="text-sm font-medium">{{ t("updates.title") }}</h3>
    <p
      class="text-sm"
      :class="status?.state === 'failed' ? 'text-cat-red' : 'text-muted-2'"
      data-updates-status
    >
      {{ line }}
    </p>
    <p v-if="status?.state === 'available' && !status.installable" class="text-xs text-muted">
      {{ t("updates.manual") }}
    </p>
    <div v-if="!off" class="flex flex-wrap gap-2">
      <button
        v-if="status?.state === 'available' && status.installable"
        type="button"
        class="flex items-center gap-2 rounded-xl bg-accent px-3 py-2 text-sm font-medium text-[#0E0F12] transition-opacity hover:opacity-90"
        data-updates-install
        @click="run(installUpdate)"
      >
        <Download :size="16" aria-hidden="true" />
        {{ t("updates.install") }}
      </button>
      <button
        v-if="status?.state === 'available'"
        type="button"
        class="flex items-center gap-2 rounded-xl border border-line-2 bg-surface-2 px-3 py-2 text-sm transition-colors hover:bg-surface-active"
        data-updates-page
        @click="run(openReleasePage)"
      >
        <ExternalLink :size="16" class="text-muted-2" aria-hidden="true" />
        {{ status.installable ? t("updates.notes") : t("updates.page") }}
      </button>
      <button
        type="button"
        class="flex items-center gap-2 rounded-xl border border-line-2 bg-surface-2 px-3 py-2 text-sm transition-colors hover:bg-surface-active disabled:pointer-events-none disabled:opacity-50"
        :disabled="busy"
        data-updates-check
        @click="run(checkForUpdates)"
      >
        <RefreshCw :size="16" class="text-muted-2" aria-hidden="true" />
        {{ t("updates.check") }}
      </button>
    </div>
    <div v-if="!off" class="flex flex-col gap-3">
      <div class="flex items-center justify-between gap-3">
        <span class="text-sm">{{ t("updates.auto") }}</span>
        <ToggleSwitch
          :checked="updates.preferences.check"
          :label="t('updates.auto')"
          data-updates-auto
          @toggle="(check) => save({ check })"
        />
      </div>
      <p class="-mt-2 text-xs text-muted">{{ t("updates.autoHint") }}</p>
      <div class="flex items-center justify-between gap-3">
        <span class="text-sm">{{ t("updates.prereleases") }}</span>
        <ToggleSwitch
          :checked="updates.preferences.prereleases"
          :label="t('updates.prereleases')"
          data-updates-prereleases
          @toggle="(prereleases) => save({ prereleases })"
        />
      </div>
      <p class="-mt-2 text-xs text-muted">{{ t("updates.prereleasesHint") }}</p>
    </div>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </div>
</template>
