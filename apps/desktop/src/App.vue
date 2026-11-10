<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import AppHeader from "./components/AppHeader.vue";
import ServerCard from "./components/ServerCard.vue";
import ClientsCard from "./components/ClientsCard.vue";
import PairedDevicesCard from "./components/PairedDevicesCard.vue";
import UsbCard from "./components/UsbCard.vue";
import ActivityCard from "./components/ActivityCard.vue";
import PairingDialog from "./components/PairingDialog.vue";
import { useStatus } from "./composables/useStatus";

const { t } = useI18n();
const { snapshot, failed } = useStatus();
const pairingOpen = ref(false);
</script>

<template>
  <div class="flex min-h-screen flex-col">
    <AppHeader :tablets="snapshot?.clients.length ?? 0" @pair="pairingOpen = true" />
    <main class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-5">
      <p v-if="failed" class="text-sm text-cat-red">{{ t("app.loadError") }}</p>
      <p v-else-if="!snapshot" class="text-sm text-muted">{{ t("app.loading") }}</p>
      <template v-else>
        <div class="grid gap-5 md:grid-cols-2">
          <ServerCard :server="snapshot.server" />
          <ClientsCard :clients="snapshot.clients" />
          <PairedDevicesCard
            :devices="snapshot.pairedDevices"
            :clients="snapshot.clients"
            @pair="pairingOpen = true"
          />
          <UsbCard :usb="snapshot.usb" :port="snapshot.server.usb.port" />
        </div>
        <ActivityCard :activity="snapshot.activity" />
        <PairingDialog
          v-if="pairingOpen"
          :pairing="snapshot.pairing"
          :wifi="snapshot.server.wifi"
          @close="pairingOpen = false"
        />
      </template>
    </main>
  </div>
</template>
