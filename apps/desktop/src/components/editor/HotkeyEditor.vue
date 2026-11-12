<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Circle, Plus, X } from "lucide-vue-next";
import { HotkeyRecorder, MAX_KEYS, normalize } from "../../editor/recorder";
import { keyLabel } from "../../editor/keys";
import { useKeys } from "../../composables/useKeys";

const props = defineProps<{ keys: string[] }>();
const emit = defineEmits<{ change: [keys: string[]] }>();
const { t, locale } = useI18n();
const validKeys = useKeys();

const recording = ref(false);
const live = ref<string[]>([]);
const adding = ref("");
let recorder: HotkeyRecorder | null = null;

const choices = computed(() =>
  validKeys.value
    .filter((k) => !props.keys.includes(k))
    .map((k) => ({ code: k, label: keyLabel(k) }))
    .sort((a, b) => a.label.localeCompare(b.label, locale.value)),
);

function onKeydown(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  recorder?.down(event.code);
  live.value = recorder?.current ?? [];
}

function onKeyup(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  const keys = recorder?.up(event.code);
  if (keys) {
    stop();
    emit("change", keys);
  }
}

// Captured on the window, before the editor's own shortcuts (undo, arrows…).
function start() {
  recorder = new HotkeyRecorder(new Set(validKeys.value));
  live.value = [];
  recording.value = true;
  window.addEventListener("keydown", onKeydown, true);
  window.addEventListener("keyup", onKeyup, true);
}

function stop() {
  recording.value = false;
  recorder = null;
  window.removeEventListener("keydown", onKeydown, true);
  window.removeEventListener("keyup", onKeyup, true);
}

onBeforeUnmount(stop);

function remove(index: number) {
  emit(
    "change",
    props.keys.filter((_, i) => i !== index),
  );
}

function add() {
  if (!adding.value) return;
  emit("change", normalize([...props.keys, adding.value]));
  adding.value = "";
}
</script>

<template>
  <div class="flex flex-col gap-3">
    <div
      class="flex min-h-[42px] flex-wrap items-center gap-1.5 rounded-[10px] border border-[#2A2D36] bg-bg p-2"
    >
      <template v-if="recording">
        <span
          v-for="k in live"
          :key="k"
          class="rounded-md border border-accent/50 bg-accent/15 px-2 py-1 text-xs"
        >
          {{ keyLabel(k) }}
        </span>
        <span class="px-1 text-xs text-muted">{{ t("hotkey.listening") }}</span>
      </template>
      <template v-else>
        <span
          v-for="(k, i) in keys"
          :key="k"
          class="flex items-center gap-1 rounded-md border border-line-2 bg-surface-2 py-1 pr-1 pl-2 text-xs"
        >
          {{ keyLabel(k) }}
          <button
            type="button"
            class="rounded p-0.5 text-muted hover:bg-surface-active hover:text-fg"
            :aria-label="t('hotkey.removeKey', { key: keyLabel(k) })"
            @click="remove(i)"
          >
            <X :size="12" aria-hidden="true" />
          </button>
        </span>
        <span v-if="keys.length === 0" class="px-1 text-xs text-[#F3D2BA]">
          {{ t("hotkey.empty") }}
        </span>
      </template>
    </div>
    <div class="flex flex-col gap-2">
      <button
        type="button"
        class="flex items-center justify-center gap-1.5 rounded-[10px] border px-3 py-2 text-sm transition-colors"
        :class="
          recording
            ? 'border-cat-red/50 bg-cat-red/10 text-[#FFC2C2]'
            : 'border-line-2 bg-surface-2 hover:bg-surface-active'
        "
        @click="recording ? stop() : start()"
      >
        <Circle
          :size="12"
          :class="recording ? 'fill-cat-red text-cat-red' : 'text-muted'"
          aria-hidden="true"
        />
        {{ recording ? t("hotkey.cancel") : t("hotkey.record") }}
      </button>
      <label class="flex min-w-0 flex-1 items-center gap-1.5">
        <span class="sr-only">{{ t("hotkey.addKey") }}</span>
        <select
          v-model="adding"
          :disabled="keys.length >= MAX_KEYS || recording"
          class="min-w-0 flex-1 rounded-[10px] border border-line-2 bg-bg px-3 py-2 text-sm disabled:opacity-40"
          @change="add"
        >
          <option value="" disabled>{{ t("hotkey.addKey") }}</option>
          <option v-for="c in choices" :key="c.code" :value="c.code">{{ c.label }}</option>
        </select>
        <Plus :size="14" class="shrink-0 text-muted" aria-hidden="true" />
      </label>
    </div>
    <p class="text-xs leading-relaxed text-muted">{{ t("hotkey.hint") }}</p>
  </div>
</template>
