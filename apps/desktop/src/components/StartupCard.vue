<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Power } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import ToggleSwitch from "./ToggleSwitch.vue";
import { getAutostart, setAutostart } from "../commands";

const { t } = useI18n();
const enabled = ref<boolean | null>(null);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    enabled.value = await getAutostart();
  } catch (e) {
    error.value = String(e);
  }
});

async function toggle(next: boolean) {
  const previous = enabled.value;
  enabled.value = next;
  error.value = null;
  try {
    await setAutostart(next);
  } catch (e) {
    enabled.value = previous;
    error.value = String(e);
  }
}
</script>

<template>
  <BaseCard :title="t('startup.title')" :icon="Power">
    <div v-if="enabled !== null" class="flex items-center justify-between gap-3">
      <span class="text-sm">{{ t("startup.toggle") }}</span>
      <ToggleSwitch
        :checked="enabled"
        :label="t('startup.toggle')"
        data-autostart-toggle
        @toggle="toggle"
      />
    </div>
    <p class="text-xs text-muted">{{ t("startup.hint") }}</p>
    <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
  </BaseCard>
</template>
