<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { TriangleAlert } from "lucide-vue-next";

const { t } = useI18n();
</script>

<template>
  <div
    role="alert"
    class="flex gap-3 rounded-2xl border border-cat-orange/40 bg-cat-orange/10 p-4 text-sm"
  >
    <TriangleAlert :size="20" class="mt-0.5 shrink-0 text-cat-orange" aria-hidden="true" />
    <div class="flex flex-col gap-1">
      <strong class="font-semibold text-cat-orange">{{ t("lan.warningTitle") }}</strong>
      <p class="text-[#F3D2BA]">{{ t("lan.warningBody") }}</p>
    </div>
  </div>
</template>
