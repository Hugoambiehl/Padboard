<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Clapperboard } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import ToggleSwitch from "./ToggleSwitch.vue";
import { setObs } from "../commands";
import { useObs } from "../composables/useObs";

const { t } = useI18n();
const { info, refresh } = useObs();

const host = ref("127.0.0.1");
const port = ref(4455);
const password = ref("");
const busy = ref(false);
const error = ref<string | null>(null);
const saved = ref(false);

// Fields follow the saved settings until edited.
watch(
  () => (info.value?.available ? [info.value.host, info.value.port] : null),
  (v) => {
    if (!v) return;
    host.value = v[0] as string;
    port.value = v[1] as number;
  },
  { immediate: true },
);

const status = computed(() => (info.value?.available ? info.value.status : null));
const dot = computed(() => {
  switch (status.value?.state) {
    case "connected":
      return "bg-cat-teal";
    case "connecting":
      return "bg-cat-orange";
    case "unreachable":
    case "authFailed":
      return "bg-cat-red";
    default:
      return "bg-[#3A3D47]";
  }
});

async function save(enabled: boolean, newPassword?: string) {
  busy.value = true;
  error.value = null;
  saved.value = false;
  try {
    await setObs(enabled, host.value, Number(port.value), newPassword);
    password.value = "";
    saved.value = true;
    await refresh();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

const enabled = computed(() => (info.value?.available ? info.value.enabled : false));
</script>

<template>
  <BaseCard :title="t('obs.title')" :icon="Clapperboard">
    <p v-if="info && !info.available" class="text-sm text-muted">{{ t("obs.unavailable") }}</p>
    <template v-else-if="info?.available">
      <div class="flex items-center justify-between gap-3">
        <span class="flex items-center gap-2 text-sm">
          <span class="size-2 shrink-0 rounded-full" :class="dot" />
          <span data-obs-status>
            {{
              status?.state === "connected"
                ? t("obs.status.connected", { version: status.version })
                : t(`obs.status.${status?.state ?? "disabled"}`)
            }}
          </span>
        </span>
        <ToggleSwitch
          :checked="enabled"
          :disabled="busy"
          :label="t('obs.toggle')"
          @toggle="(v) => save(v, password || undefined)"
        />
      </div>
      <form class="flex flex-col gap-3" @submit.prevent="save(enabled, password || undefined)">
        <div class="flex gap-2">
          <label class="flex flex-1 flex-col gap-1.5 text-xs text-muted">
            {{ t("obs.host") }}
            <input v-model="host" class="field" maxlength="253" />
          </label>
          <label class="flex w-24 flex-col gap-1.5 text-xs text-muted">
            {{ t("obs.port") }}
            <input v-model.number="port" class="field" type="number" min="1" max="65535" />
          </label>
        </div>
        <label class="flex flex-col gap-1.5 text-xs text-muted">
          {{ t("obs.password") }}
          <input
            v-model="password"
            class="field"
            type="password"
            autocomplete="off"
            :placeholder="info.hasPassword ? t('obs.passwordSaved') : ''"
          />
        </label>
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="submit"
            class="rounded-[10px] bg-accent px-3.5 py-2 text-sm font-semibold text-bg hover:opacity-90 disabled:opacity-50"
            :disabled="busy"
          >
            {{ t("obs.save") }}
          </button>
          <button
            v-if="info.hasPassword"
            type="button"
            class="rounded-[10px] border border-line-2 px-3 py-2 text-sm hover:bg-surface-active"
            :disabled="busy"
            @click="save(enabled, '')"
          >
            {{ t("obs.removePassword") }}
          </button>
          <span v-if="saved" class="text-xs text-cat-teal">{{ t("obs.saved") }}</span>
          <span v-if="error" class="text-xs text-cat-red">{{ error }}</span>
        </div>
      </form>
      <p class="text-xs leading-relaxed text-muted">{{ t("obs.hint") }}</p>
    </template>
  </BaseCard>
</template>

<style scoped>
@reference "../style.css";
.field {
  @apply rounded-[10px] border border-[#2A2D36] bg-bg px-3 py-2 text-sm text-fg outline-none focus:border-accent;
}
</style>
