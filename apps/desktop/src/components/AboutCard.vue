<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { Info, Scale } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import CopyField from "./CopyField.vue";
import LicensesDialog from "./LicensesDialog.vue";
import UpdatesSection from "./UpdatesSection.vue";

const SOURCE_URL = "https://github.com/Hugoambiehl/Padboard";

const { t } = useI18n();
const version = ref<string | null>(null);
const licensesOpen = ref(false);

onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch (e) {
    console.warn("version unavailable", e);
  }
});
</script>

<template>
  <BaseCard id="about" :title="t('about.title')" :icon="Info">
    <p class="text-sm">
      {{ t("app.name") }}
      <span v-if="version" class="text-muted-2">{{ t("about.version", { version }) }}</span>
    </p>
    <p class="text-sm text-muted-2">{{ t("about.license") }}</p>
    <p class="text-xs text-muted">{{ t("about.warranty") }}</p>
    <div class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("about.source") }}
      <CopyField :value="SOURCE_URL" />
    </div>
    <button
      type="button"
      class="flex items-center gap-2 self-start rounded-xl border border-line-2 bg-surface-2 px-3 py-2 text-sm transition-colors hover:bg-surface-active"
      data-licenses-open
      @click="licensesOpen = true"
    >
      <Scale :size="16" class="text-muted-2" aria-hidden="true" />
      {{ t("about.licenses") }}
    </button>
    <UpdatesSection />
    <LicensesDialog v-if="licensesOpen" @close="licensesOpen = false" />
  </BaseCard>
</template>
