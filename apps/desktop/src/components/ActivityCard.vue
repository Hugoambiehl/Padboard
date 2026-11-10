<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Activity, CircleCheck, CircleX } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import type { ActivityEntry } from "../types/status";

defineProps<{ activity: ActivityEntry[] }>();
const { t, d } = useI18n();
</script>

<template>
  <BaseCard :title="t('activity.title')" :icon="Activity">
    <p v-if="activity.length === 0" class="text-sm text-muted">{{ t("activity.empty") }}</p>
    <ol v-else class="flex flex-col divide-y divide-line">
      <li v-for="a in activity" :key="a.id" class="flex items-center gap-3 py-2 text-sm">
        <span class="w-20 shrink-0 font-mono text-xs text-muted">
          {{ d(new Date(a.atMs), "time") }}
        </span>
        <span class="min-w-0 flex-1 truncate font-medium">
          {{ a.label ?? t("activity.unknownButton", { id: a.buttonId }) }}
        </span>
        <span class="hidden truncate text-xs text-muted sm:block">
          {{ a.deviceName ?? t("activity.unknownDevice") }}
        </span>
        <span
          v-if="a.outcome.status === 'ok'"
          class="flex shrink-0 items-center gap-1.5 text-cat-teal"
        >
          <CircleCheck :size="15" aria-hidden="true" />
          {{ t("activity.ok") }}
        </span>
        <span
          v-else
          class="flex shrink-0 items-center gap-1.5 text-cat-red"
          :title="a.outcome.detail"
        >
          <CircleX :size="15" aria-hidden="true" />
          {{ t(`activity.errors.${a.outcome.code}`) }}
        </span>
      </li>
    </ol>
  </BaseCard>
</template>
