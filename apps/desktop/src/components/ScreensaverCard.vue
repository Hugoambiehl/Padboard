<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { MoonStar } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import ToggleSwitch from "./ToggleSwitch.vue";
import { getScreensaver, setScreensaver, type Screensaver } from "../commands";

const { t } = useI18n();

/** Delays offered, in seconds (the backend accepts 15 s to 1 h). */
const delays = [30, 60, 120, 300, 600, 900, 1800, 3600];

const settings = ref<Screensaver | null>(null);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    settings.value = await getScreensaver();
  } catch (e) {
    error.value = String(e);
  }
});

async function save(next: Screensaver) {
  const previous = settings.value;
  settings.value = next;
  error.value = null;
  try {
    await setScreensaver(next);
  } catch (e) {
    settings.value = previous;
    error.value = String(e);
  }
}

function delayLabel(seconds: number): string {
  return seconds < 60
    ? t("screensaver.seconds", { n: seconds })
    : t("screensaver.minutes", { n: seconds / 60 });
}
</script>

<template>
  <BaseCard :title="t('screensaver.title')" :icon="MoonStar">
    <template v-if="settings">
      <div class="flex items-center justify-between gap-3">
        <span class="text-sm">{{ t("screensaver.toggle") }}</span>
        <ToggleSwitch
          :checked="settings.enabled"
          :label="t('screensaver.toggle')"
          data-screensaver-toggle
          @toggle="(enabled) => save({ ...settings!, enabled })"
        />
      </div>
      <label class="flex flex-col gap-1.5 text-xs text-muted">
        {{ t("screensaver.delay") }}
        <select
          class="control"
          :value="settings.delaySeconds"
          :disabled="!settings.enabled"
          data-screensaver-delay
          @change="
            save({
              ...settings!,
              delaySeconds: Number(($event.target as HTMLSelectElement).value),
            })
          "
        >
          <option v-if="!delays.includes(settings.delaySeconds)" :value="settings.delaySeconds">
            {{ delayLabel(settings.delaySeconds) }}
          </option>
          <option v-for="d in delays" :key="d" :value="d">{{ delayLabel(d) }}</option>
        </select>
      </label>
      <p class="text-xs text-muted">{{ t("screensaver.hint") }}</p>
    </template>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
