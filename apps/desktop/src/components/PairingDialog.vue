<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CircleCheck, Link2, ShieldCheck, TriangleAlert, WifiOff, X } from "lucide-vue-next";
import { cancelPairing, setWifiEnabled, startPairing } from "../commands";
import { useNow } from "../composables/useNow";
import type { PairingView, WifiStatus } from "../types/status";

const props = defineProps<{ pairing: PairingView; wifi: WifiStatus }>();
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
const now = useNow(250);

const dialog = ref<HTMLDialogElement | null>(null);
const error = ref<string | null>(null);
const busy = ref(false);

const wifiReady = computed(() => props.wifi.enabled && props.wifi.listener.state === "listening");
/** Wi-Fi forced off by the test mode: its message alone, no pairing texts. */
const testMode = computed(() => props.wifi.forcedByTestMode && !props.wifi.enabled);
const canEnableWifi = computed(() => props.wifi.forced === null && props.wifi.tlsError === null);

/** Seconds left on the active PIN; 0 once expired (the backend only notices on the next attempt). */
const secondsLeft = computed(() =>
  props.pairing.state === "active"
    ? Math.max(0, Math.ceil((props.pairing.expiresAtMs - now.value) / 1000))
    : 0,
);
const view = computed(() =>
  props.pairing.state === "active" && secondsLeft.value === 0 ? "expired" : props.pairing.state,
);
const countdown = computed(() => {
  const s = secondsLeft.value;
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
});
const digits = computed(() =>
  props.pairing.state === "active" ? props.pairing.pin.split("") : [],
);

async function newPin() {
  error.value = null;
  busy.value = true;
  try {
    await startPairing();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function enableWifiThenPair() {
  error.value = null;
  busy.value = true;
  try {
    await setWifiEnabled(true);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

// A PIN is only shown while this dialog is open, and only once Wi-Fi listens.
watch(
  wifiReady,
  (ready) => {
    if (ready && props.pairing.state !== "active") void newPin();
  },
  { immediate: true },
);

function close() {
  if (props.pairing.state === "active") void cancelPairing();
  emit("close");
}

onMounted(() => dialog.value?.showModal());
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto w-[min(30rem,calc(100vw-2rem))] rounded-2xl border border-line bg-surface p-0 text-fg backdrop:bg-black/60"
    aria-labelledby="pairing-title"
    @cancel.prevent="close"
  >
    <div class="flex flex-col gap-5 p-6">
      <header class="flex items-start gap-3">
        <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-accent/15">
          <Link2 :size="18" class="text-accent" aria-hidden="true" />
        </span>
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <h2 id="pairing-title" class="font-display text-lg font-semibold">
            {{ t("pairing.title") }}
          </h2>
          <p v-if="!testMode" class="text-sm text-muted">{{ t("pairing.intro") }}</p>
        </div>
        <button
          type="button"
          class="rounded-lg p-1.5 text-muted transition-colors hover:bg-surface-active hover:text-fg"
          :aria-label="t('pairing.close')"
          @click="close"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <!-- Wi-Fi off: pairing needs it. -->
      <div v-if="!wifiReady" class="flex flex-col items-start gap-3">
        <p v-if="testMode" class="flex gap-2 text-sm text-muted-2">
          <WifiOff :size="16" class="mt-0.5 shrink-0" aria-hidden="true" />
          {{ t("pairing.wifiLockedTest") }}
        </p>
        <p v-else class="flex gap-2 text-sm text-muted-2">
          <WifiOff :size="16" class="mt-0.5 shrink-0" aria-hidden="true" />
          {{ wifi.enabled ? t("pairing.wifiStarting") : t("pairing.wifiOff") }}
        </p>
        <button
          v-if="!wifi.enabled && canEnableWifi"
          type="button"
          class="rounded-xl bg-accent px-4 py-2 text-sm font-semibold text-bg transition-opacity hover:opacity-90 disabled:opacity-50"
          :disabled="busy"
          @click="enableWifiThenPair"
        >
          {{ t("pairing.enableWifi") }}
        </button>
        <p v-else-if="!wifi.enabled && !testMode" class="text-xs text-muted">
          {{ t("pairing.wifiLocked") }}
        </p>
      </div>

      <!-- Active PIN -->
      <template v-else-if="view === 'active'">
        <div class="flex flex-col items-center gap-3">
          <span class="text-sm text-muted">{{ t("pairing.codeLabel") }}</span>
          <div class="flex gap-2" role="group" :aria-label="t('pairing.codeLabel')">
            <span
              v-for="(digit, i) in digits"
              :key="i"
              class="flex h-16 w-12 items-center justify-center rounded-xl border border-line-2 bg-surface-2 font-display text-[32px] font-semibold tabular-nums"
              :class="i === 3 ? 'ml-3' : ''"
            >
              {{ digit }}
            </span>
          </div>
          <span class="text-sm text-muted tabular-nums">
            {{ t("pairing.expiresIn", { time: countdown }) }}
            <template v-if="pairing.state === 'active' && pairing.attemptsLeft < 5">
              · {{ t("pairing.attemptsLeft", pairing.attemptsLeft) }}
            </template>
          </span>
        </div>

        <div
          v-if="wifi.fingerprint"
          class="flex flex-col gap-2 rounded-xl border border-line-2 bg-surface-2 p-4"
        >
          <p class="flex items-center gap-2 text-sm font-semibold">
            <ShieldCheck :size="16" class="text-cat-teal" aria-hidden="true" />
            {{ t("pairing.checkTitle") }}
          </p>
          <code class="text-center font-mono text-xl tracking-wider">{{ wifi.fingerprint }}</code>
          <p class="text-xs text-muted">{{ t("pairing.checkHint") }}</p>
        </div>
      </template>

      <!-- Expired / locked: a new PIN is required. -->
      <div
        v-else-if="view === 'expired' || view === 'locked'"
        class="flex flex-col items-start gap-3"
      >
        <p class="flex gap-2 text-sm text-[#F3D2BA]">
          <TriangleAlert :size="16" class="mt-0.5 shrink-0 text-cat-orange" aria-hidden="true" />
          {{ view === "expired" ? t("pairing.expired") : t("pairing.locked") }}
        </p>
        <button
          type="button"
          class="rounded-xl bg-accent px-4 py-2 text-sm font-semibold text-bg transition-opacity hover:opacity-90 disabled:opacity-50"
          :disabled="busy"
          @click="newPin"
        >
          {{ t("pairing.newCode") }}
        </button>
      </div>

      <!-- Paired -->
      <div v-else-if="pairing.state === 'paired'" class="flex flex-col items-start gap-3">
        <p class="flex gap-2 text-sm">
          <CircleCheck :size="16" class="mt-0.5 shrink-0 text-cat-teal" aria-hidden="true" />
          {{ t("pairing.paired", { name: pairing.deviceName }) }}
        </p>
        <button
          type="button"
          class="rounded-xl border border-line-2 bg-surface-2 px-4 py-2 text-sm transition-colors hover:bg-surface-active"
          @click="close"
        >
          {{ t("pairing.done") }}
        </button>
      </div>

      <p v-else class="text-sm text-muted">{{ t("app.loading") }}</p>

      <p v-if="error" class="text-sm text-cat-red">{{ error }}</p>
    </div>
  </dialog>
</template>
