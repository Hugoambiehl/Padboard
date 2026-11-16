<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { Cable, CircleHelp, Download, FolderOpen, Lock, RotateCcw, Trash2 } from "lucide-vue-next";
import BaseCard from "./BaseCard.vue";
import PlatformToolsDialog from "./PlatformToolsDialog.vue";
import {
  enableKiosk,
  openHelp,
  platformToolsInfo,
  removePlatformTools,
  setAdbPath,
  updateUsbTablet,
} from "../commands";
import type { PlatformToolsInfo } from "../types/platformTools";
import type { DeviceState, UsbStatus } from "../types/status";
import { kioskActiveKey, kioskErrorMessage } from "../usb/kiosk";
import { devMismatch, needsUpdate, usbUpdateErrorMessage } from "../usb/update";

const props = defineProps<{
  usb: UsbStatus;
  port: number;
  /** This PC's version, offered to older tablets. */
  appVersion: string;
  appVersionCode: number | null;
  /** A development version, and the versionCode of its own tablet builds. */
  development: boolean;
  developmentCode: number | null;
}>();
const { t } = useI18n();
const error = ref<string | null>(null);

/**
 * Kiosk mode being turned on (every button is disabled meanwhile), and the
 * last refusal per device. Success shows through the device's kiosk state,
 * read again by the backend right after.
 */
const enabling = ref<string | null>(null);
const kioskError = ref<Record<string, string>>({});

async function kiosk(serial: string) {
  if (enabling.value) return;
  enabling.value = serial;
  kioskError.value = Object.fromEntries(
    Object.entries(kioskError.value).filter(([key]) => key !== serial),
  );
  try {
    await enableKiosk(serial);
  } catch (e) {
    const { key, params } = kioskErrorMessage(e);
    kioskError.value = { ...kioskError.value, [serial]: t(key, params) };
  } finally {
    enabling.value = null;
  }
}

/**
 * USB updates: the device being updated (one at a time, like kiosk mode),
 * and the last outcome per device (installed version, or why not).
 */
const updating = ref<string | null>(null);
const updateOutcome = ref<Record<string, { ok: boolean; text: string }>>({});

async function update(serial: string) {
  if (updating.value) return;
  updating.value = serial;
  updateOutcome.value = Object.fromEntries(
    Object.entries(updateOutcome.value).filter(([key]) => key !== serial),
  );
  try {
    const version = await updateUsbTablet(serial);
    updateOutcome.value = {
      ...updateOutcome.value,
      [serial]: { ok: true, text: t("tabletUpdate.updated", { version }) },
    };
  } catch (e) {
    const { key, params } = usbUpdateErrorMessage(e);
    updateOutcome.value = { ...updateOutcome.value, [serial]: { ok: false, text: t(key, params) } };
  } finally {
    updating.value = null;
  }
}

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

/** Google's Platform-Tools, offered on Windows when adb is missing. */
const tools = ref<PlatformToolsInfo | null>(null);
const toolsDialog = ref(false);
const removing = ref(false);
onMounted(async () => {
  try {
    tools.value = await platformToolsInfo();
  } catch {
    tools.value = null;
  }
});
const offerTools = computed(
  () => tools.value?.offered === true && adb.value.state === "missing" && !chosenMissing.value,
);
/** adb in use is the copy Padboard downloaded. */
const usingTools = computed(
  () =>
    tools.value?.installed != null &&
    adb.value.state !== "missing" &&
    adb.value.state !== "searching" &&
    adb.value.path === tools.value.installed,
);

function toolsInstalled(info: PlatformToolsInfo) {
  tools.value = info;
  toolsDialog.value = false;
}

async function chooseInstead() {
  toolsDialog.value = false;
  await choose();
}

async function removeTools() {
  if (removing.value) return;
  removing.value = true;
  error.value = null;
  try {
    tools.value = await removePlatformTools();
  } catch (e) {
    error.value = typeof e === "object" && e && "detail" in e ? String(e.detail) : String(e);
  } finally {
    removing.value = false;
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
              : tools?.offered
                ? t("usbDevices.adb.missingWindows")
                : t("usbDevices.adb.missing")
          }}
        </p>
        <div v-if="offerTools" class="flex flex-col gap-2" data-platform-tools-offer>
          <p class="text-xs text-muted">
            {{ t("platformTools.offer", { version: tools!.version }) }}
          </p>
          <button
            type="button"
            class="flex items-center gap-1.5 self-start rounded-lg bg-accent px-2.5 py-1.5 text-xs font-medium text-[#0E0F12] transition-opacity hover:opacity-90"
            data-platform-tools-open
            @click="toolsDialog = true"
          >
            <Download :size="14" aria-hidden="true" />
            {{ t("platformTools.button") }}
          </button>
        </div>
      </template>
      <div
        v-else-if="adb.state === 'conflict'"
        class="flex flex-col gap-1 rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-[#F3D2BA]"
        data-adb-conflict
      >
        <p>{{ t("usbDevices.adb.conflict", { client: adb.client, server: adb.server }) }}</p>
        <p class="text-xs opacity-90">{{ t("usbDevices.adb.conflictHint") }}</p>
        <span class="truncate font-mono text-xs opacity-80" :title="adb.path">{{ adb.path }}</span>
      </div>
      <p v-else-if="adb.state === 'error'" class="text-muted">
        {{ t("usbDevices.adb.error") }}
        <span class="mt-1 block font-mono text-xs opacity-80">{{ adb.detail }}</span>
      </p>
      <p v-else class="flex flex-col text-muted">
        <span>{{ adb.version ?? "adb" }}</span>
        <span class="truncate font-mono text-xs" :title="adb.path">{{ adb.path }}</span>
        <span v-if="usingTools" class="text-xs" data-platform-tools-installed>
          {{ t("platformTools.inUse", { version: tools!.version }) }}
        </span>
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
          v-if="usingTools"
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs transition-colors hover:bg-surface-active disabled:cursor-wait disabled:opacity-50"
          :disabled="removing"
          :title="t('platformTools.removeHint')"
          data-platform-tools-remove
          @click="removeTools"
        >
          <Trash2 :size="14" aria-hidden="true" />
          {{ t("platformTools.remove") }}
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
      <p
        v-if="usb.serverUnstable"
        class="rounded-xl border border-cat-orange/40 bg-cat-orange/10 p-3 text-xs text-[#F3D2BA]"
        data-adb-unstable
      >
        {{ t("usbDevices.adb.unstable") }}
      </p>
      <p v-if="error" class="text-cat-red">{{ error }}</p>
    </div>

    <!-- Devices -->
    <div class="flex flex-col gap-2 border-t border-line pt-4">
      <p v-if="usb.devices.length === 0" class="text-sm text-muted">
        {{ adb.state === "ready" ? t("usbDevices.none") : t("usbDevices.noneWithoutAdb") }}
      </p>
      <button
        v-if="usb.devices.length === 0 && adb.state === 'ready' && tools?.offered"
        type="button"
        class="flex items-center gap-1.5 self-start text-xs text-muted-2 hover:text-fg hover:underline"
        data-usb-driver-help
        @click="openHelp('usbDriver')"
      >
        <CircleHelp :size="13" aria-hidden="true" />
        {{ t("usbDevices.driverHelp") }}
      </button>
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
            <span v-if="device.app?.name" class="text-xs text-muted-2" data-app-version>
              {{ t("tabletUpdate.version", { version: device.app.name }) }}
            </span>
            <button
              v-if="
                (device.state.state === 'ready' || device.state.state === 'failed') &&
                needsUpdate(device.app?.code, appVersionCode)
              "
              type="button"
              class="mt-1.5 flex items-center gap-1.5 self-start rounded-lg bg-accent px-2.5 py-1.5 text-xs font-medium text-[#0E0F12] transition-opacity hover:opacity-90 disabled:cursor-wait disabled:opacity-50"
              :disabled="updating !== null || enabling !== null"
              :aria-busy="updating === device.serial"
              :title="t('tabletUpdate.hint')"
              data-usb-update
              @click="update(device.serial)"
            >
              <Download :size="13" aria-hidden="true" />
              {{
                updating === device.serial
                  ? t("tabletUpdate.updating")
                  : t("tabletUpdate.update", { version: appVersion })
              }}
            </button>
            <span
              v-if="devMismatch(device.app?.code, development, developmentCode)"
              class="mt-1 text-xs text-cat-orange"
              data-usb-dev-mismatch
            >
              {{
                t("tabletUpdate.devMismatch", { pc: appVersion, tablet: device.app?.name ?? "?" })
              }}
            </span>
            <span
              v-if="updateOutcome[device.serial]"
              class="text-xs"
              :class="updateOutcome[device.serial]!.ok ? 'text-cat-teal' : 'text-cat-red'"
              data-usb-update-outcome
            >
              {{ updateOutcome[device.serial]!.text }}
            </span>
            <template v-if="device.state.state === 'ready'">
              <span
                v-if="device.kiosk.state === 'on'"
                class="mt-1 flex items-start gap-1.5 text-xs text-cat-teal"
                data-kiosk="on"
              >
                <Lock :size="12" class="mt-0.5 shrink-0" aria-hidden="true" />
                {{ t(kioskActiveKey(device.app)) }}
              </span>
              <span
                v-else-if="device.kiosk.state === 'otherOwner'"
                class="mt-1 text-xs text-cat-orange"
                data-kiosk="otherOwner"
              >
                {{ t("usbDevices.kiosk.otherOwner", { owner: device.kiosk.owner }) }}
              </span>
              <button
                v-else
                type="button"
                class="mt-1.5 self-start rounded-lg border border-line-2 bg-surface px-2.5 py-1.5 text-xs text-muted-2 transition-colors hover:bg-surface-active hover:text-fg disabled:cursor-wait disabled:opacity-50"
                :disabled="enabling !== null || updating !== null"
                :aria-busy="enabling === device.serial"
                :title="t('usbDevices.kiosk.hint')"
                @click="kiosk(device.serial)"
              >
                {{
                  enabling === device.serial
                    ? t("usbDevices.kiosk.enabling")
                    : t("usbDevices.kiosk.enable")
                }}
              </button>
              <span v-if="kioskError[device.serial]" class="text-xs text-cat-red">
                {{ kioskError[device.serial] }}
              </span>
            </template>
          </div>
        </li>
      </ul>
    </div>
    <PlatformToolsDialog
      v-if="toolsDialog && tools"
      :version="tools.version"
      @close="toolsDialog = false"
      @installed="toolsInstalled"
      @choose="chooseInstead"
    />
  </BaseCard>
</template>
