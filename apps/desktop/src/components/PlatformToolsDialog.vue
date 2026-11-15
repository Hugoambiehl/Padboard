<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ExternalLink, FolderOpen } from "lucide-vue-next";
import { installPlatformTools, openHelp } from "../commands";
import type { PlatformToolsError, PlatformToolsInfo } from "../types/platformTools";

/**
 * "Installer adb" (Windows): Google's Platform-Tools are not free software,
 * so the user reads what their license says and accepts it before Padboard
 * downloads them from Google. Optional: Wi-Fi works without adb.
 */
const props = defineProps<{ version: string }>();
const emit = defineEmits<{
  close: [];
  installed: [info: PlatformToolsInfo];
  /** The pinned version is gone: choose adb by hand instead. */
  choose: [];
}>();
const { t } = useI18n();
const dialog = ref<HTMLDialogElement | null>(null);
const accepted = ref(false);
const installing = ref(false);
const failure = ref<PlatformToolsError | null>(null);

onMounted(() => dialog.value?.showModal());

const failureText = computed(() => {
  const f = failure.value;
  if (!f) return "";
  switch (f.code) {
    case "unavailable":
      return t("platformTools.error.unavailable", { version: props.version });
    case "network":
    case "install":
      return t(`platformTools.error.${f.code}`, { detail: f.detail });
    case "signature":
      return t("platformTools.error.signature", { file: f.file });
    default:
      return t(`platformTools.error.${f.code}`);
  }
});

function close() {
  if (!installing.value) emit("close");
}

async function install() {
  if (!accepted.value || installing.value) return;
  installing.value = true;
  failure.value = null;
  try {
    emit("installed", await installPlatformTools(true));
  } catch (e) {
    failure.value =
      typeof e === "object" && e !== null && "code" in e
        ? (e as PlatformToolsError)
        : { code: "install", detail: String(e) };
  } finally {
    installing.value = false;
  }
}
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto max-h-[calc(100vh-2rem)] w-[min(34rem,calc(100vw-2rem))] flex-col gap-4 overflow-y-auto rounded-2xl border border-line bg-surface p-6 text-fg open:flex backdrop:bg-black/60"
    aria-labelledby="platform-tools-title"
    data-platform-tools-dialog
    @cancel.prevent="close"
  >
    <h2 id="platform-tools-title" class="font-display text-lg font-semibold">
      {{ t("platformTools.title") }}
    </h2>
    <p class="text-sm text-muted-2">{{ t("platformTools.optional") }}</p>
    <p class="text-sm text-muted-2">
      {{ t("platformTools.what", { version }) }}
    </p>
    <div class="flex flex-col gap-2 rounded-xl border border-line-2 bg-surface-2 p-3.5 text-sm">
      <p class="font-medium">{{ t("platformTools.licenseTitle") }}</p>
      <ul class="flex list-disc flex-col gap-1.5 pl-5 text-muted-2">
        <li>{{ t("platformTools.license31") }}</li>
        <li>{{ t("platformTools.license34") }}</li>
        <li>{{ t("platformTools.notFree") }}</li>
        <li>{{ t("platformTools.notAffiliated") }}</li>
      </ul>
      <button
        type="button"
        class="flex items-center gap-1.5 self-start text-accent hover:underline"
        data-platform-tools-license
        @click="openHelp('sdkLicense')"
      >
        <ExternalLink :size="14" aria-hidden="true" />
        {{ t("platformTools.readLicense") }}
      </button>
    </div>
    <p class="text-xs text-muted">{{ t("platformTools.checks") }}</p>
    <label class="flex items-start gap-2.5 text-sm">
      <input
        v-model="accepted"
        type="checkbox"
        class="mt-0.5 size-4 accent-[#8B7CFF]"
        :disabled="installing"
        data-platform-tools-accept
      />
      <span>{{ t("platformTools.accept") }}</span>
    </label>
    <div
      v-if="failure"
      class="flex flex-col gap-2 rounded-xl border border-cat-red/40 bg-cat-red/10 p-3 text-sm"
      data-platform-tools-error
    >
      <p class="text-[#F5C2C2]">{{ failureText }}</p>
      <div v-if="failure.code === 'unavailable'" class="flex flex-wrap gap-2">
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs transition-colors hover:bg-surface-active"
          data-platform-tools-choose
          @click="emit('choose')"
        >
          <FolderOpen :size="14" aria-hidden="true" />
          {{ t("usbDevices.choose") }}
        </button>
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-line-2 bg-surface-2 px-2.5 py-1.5 text-xs transition-colors hover:bg-surface-active"
          @click="openHelp('platformTools')"
        >
          <ExternalLink :size="14" aria-hidden="true" />
          {{ t("platformTools.googlePage") }}
        </button>
      </div>
    </div>
    <div class="flex flex-wrap justify-end gap-2.5">
      <button
        type="button"
        class="rounded-[10px] border border-line-2 bg-surface-2 px-3.5 py-2 text-sm transition-colors hover:bg-surface-active disabled:opacity-50"
        :disabled="installing"
        data-platform-tools-cancel
        @click="close"
      >
        {{ t("platformTools.cancel") }}
      </button>
      <button
        type="button"
        class="rounded-[10px] bg-accent px-3.5 py-2 text-sm font-medium text-bg transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="!accepted || installing"
        :aria-busy="installing"
        data-platform-tools-install
        @click="install"
      >
        {{ installing ? t("platformTools.installing") : t("platformTools.install") }}
      </button>
    </div>
  </dialog>
</template>
