<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowLeft } from "lucide-vue-next";
import ServerCard from "../components/ServerCard.vue";
import ClientsCard from "../components/ClientsCard.vue";
import PairedDevicesCard from "../components/PairedDevicesCard.vue";
import UsbCard from "../components/UsbCard.vue";
import ObsCard from "../components/ObsCard.vue";
import ScreensaverCard from "../components/ScreensaverCard.vue";
import ActivityCard from "../components/ActivityCard.vue";
import StartupCard from "../components/StartupCard.vue";
import AboutCard from "../components/AboutCard.vue";
import type { Snapshot } from "../types/status";

defineProps<{ snapshot: Snapshot }>();
defineEmits<{ back: []; pair: [] }>();
const { t } = useI18n();
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-5">
    <button
      type="button"
      class="flex items-center gap-2 self-start rounded-lg px-2 py-1 text-sm text-muted-2 hover:bg-surface-active hover:text-fg"
      @click="$emit('back')"
    >
      <ArrowLeft :size="16" aria-hidden="true" />
      {{ t("app.backToEditor") }}
    </button>
    <div class="grid gap-5 md:grid-cols-2">
      <ServerCard :server="snapshot.server" />
      <ClientsCard
        :clients="snapshot.clients"
        :outdated="snapshot.outdatedTablets"
        :app-version="snapshot.server.appVersion"
        :app-version-code="snapshot.server.appVersionCode"
      />
      <PairedDevicesCard
        :devices="snapshot.pairedDevices"
        :clients="snapshot.clients"
        @pair="$emit('pair')"
      />
      <UsbCard
        :usb="snapshot.usb"
        :port="snapshot.server.usb.port"
        :app-version="snapshot.server.appVersion"
        :app-version-code="snapshot.server.appVersionCode"
      />
      <ObsCard />
      <ScreensaverCard />
      <StartupCard />
      <AboutCard />
    </div>
    <ActivityCard :activity="snapshot.activity" />
  </div>
</template>
