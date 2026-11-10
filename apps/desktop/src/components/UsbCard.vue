<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { Cable, FolderOpen, RotateCcw } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import { setAdbPath } from "../commands";
import type { DeviceState, UsbStatus } from "../types/status";

const props = defineProps<{ usb: UsbStatus; port: number }>();
const { t } = useI18n();
const error = ref<string | null>(null);

const adb = computed(() => props.usb.adb);
/** A path chosen by the user (missing) or in use; `chosen` tells which. */
const chosenMissing = computed(() => (adb.value.state === "missing" ? adb.value.chosen : null));

const tone: Record<DeviceState["state"], string> = {
  ready: "bg-cat-teal",
  checking: "bg-cat-orange",
  unauthorized: "bg-cat-orange",
  noPermission: "bg-cat-red",
  failed: "bg-cat-red",
  offline: "bg-[#3A3D47]",
  noPadboard: "bg-[#3A3D47]",
  other: "bg-[#3A3D47]",
};

async function choose() {
  error.value = null;
  try {
    const path = await open({
      title: t("usbDevices.chooseTitle"),
      multiple: false,
      directory: false,
    });
    if (typeof path === "string") await setAdbPath(path);
  } catch (e) {
    error.value = String(e);
  }
}

async function automatic() {
  error.value = null;
  try {
    await setAdbPath(null);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <BaseCard :title="t('usbDevices.title')" :icon="Cable">
    <!-- adb -->
    <div class="flex flex-col gap-2 text-sm">
      <p v-if="adb.state === 'searching'" class="text-muted">{{ t("usbDevices.adb.searching") }}</p>
      <template v-else-if="adb.state === 'missing'">
        <p class="rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-[#F3D2BA]">
          {{
            chosenMissing
              ? t("usbDevices.adb.chosenMissing", { path: chosenMissing })
              : t("usbDevices.adb.missing")
          }}
        </p>
      </template>
      <p v-else-if="adb.state === 'error'" class="text-muted">
        {{ t("usbDevices.adb.error") }}
        <span class="mt-1 block font-mono text-xs opacity-80">{{ adb.detail }}</span>
      </p>
      <p v-else class="flex flex-col text-muted">
        <span>{{ adb.version ?? "adb" }}</span>
        <span class="truncate font-mono text-xs" :title="adb.path">{{ adb.path }}</span>
      </p>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs transition-colors hover:bg-surface-active"
          @click="choose"
        >
          <FolderOpen :size="14" aria-hidden="true" />
          {{ t("usbDevices.choose") }}
        </button>
        <button
          v-if="chosenMissing"
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs transition-colors hover:bg-surface-active"
          @click="automatic"
        >
          <RotateCcw :size="14" aria-hidden="true" />
          {{ t("usbDevices.automatic") }}
        </button>
      </div>
      <p v-if="error" class="text-cat-red">{{ error }}</p>
    </div>

    <!-- Devices -->
    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <p v-if="usb.devices.length === 0" class="text-sm text-muted">
        {{ adb.state === "ready" ? t("usbDevices.none") : t("usbDevices.noneWithoutAdb") }}
      </p>
      <ul v-else class="flex flex-col gap-2">
        <li
          v-for="device in usb.devices"
          :key="device.serial"
          class="flex items-start gap-3 rounded-xl border border-line-2 bg-surface-2 px-3 py-2.5"
        >
          <span class="mt-1.5 size-2 shrink-0 rounded-full" :class="tone[device.state.state]" />
          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="truncate text-sm font-medium">
              {{ device.model ?? device.serial }}
              <span v-if="device.model" class="font-mono text-xs font-normal text-muted">
                {{ device.serial }}
              </span>
            </span>
            <span class="text-xs text-muted">
              <template v-if="device.state.state === 'failed'">
                {{ t("usbDevices.state.failed") }}
                <span class="block font-mono opacity-80">{{ device.state.detail }}</span>
              </template>
              <template v-else-if="device.state.state === 'other'">
                {{ t("usbDevices.state.other", { state: device.state.adbState }) }}
              </template>
              <template v-else>
                {{ t(`usbDevices.state.${device.state.state}`, { port }) }}
              </template>
            </span>
          </div>
        </li>
      </ul>
    </div>
  </BaseCard>
</template>
