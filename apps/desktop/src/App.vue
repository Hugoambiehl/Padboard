<script setup lang="ts">
import { useI18n } from "vue-i18n";
import AppHeader from "./components/AppHeader.vue";
import LanWarning from "./components/LanWarning.vue";
import ServerCard from "./components/ServerCard.vue";
import ClientsCard from "./components/ClientsCard.vue";
import ActivityCard from "./components/ActivityCard.vue";
import { useStatus } from "./composables/useStatus";

const { t } = useI18n();
const { snapshot, failed } = useStatus();
</script>

<template>
  <div class="flex min-h-screen flex-col">
    <AppHeader :tablets="snapshot?.clients.length ?? 0" />
    <main class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-5">
      <p v-if="failed" class="text-sm text-cat-red">{{ t("app.loadError") }}</p>
      <p v-else-if="!snapshot" class="text-sm text-muted">{{ t("app.loading") }}</p>
      <template v-else>
        <LanWarning v-if="snapshot.server.bindMode === 'lan'" />
        <div class="grid gap-5 md:grid-cols-2">
          <ServerCard :server="snapshot.server" />
          <ClientsCard :clients="snapshot.clients" />
        </div>
        <ActivityCard :activity="snapshot.activity" />
      </template>
    </main>
  </div>
</template>
