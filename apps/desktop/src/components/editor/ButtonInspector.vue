<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Ban, MoreHorizontal, Play, Plus } from "lucide-vue-next";
import IconPaths from "./IconPaths.vue";
import IconPicker from "./IconPicker.vue";
import HotkeyEditor from "./HotkeyEditor.vue";
import DraftInput from "./DraftInput.vue";
import { useEditor } from "../../composables/useEditor";
import { useSelection } from "../../editor/selection";
import { categories, categoryColors } from "../../editor/colors";
import { loadIcons } from "../../icons/catalog";
import { iconToPaths } from "../../icons/paths";
import { addPreset } from "../../editor/actions";
import { testButton } from "../../commands";
import { actionSummary, isSystemAction } from "../../editor/actionText";
import type { Action, Category, IconConfig } from "../../generated/types";

const { t } = useI18n();
const { edit, checkpoint } = useEditor();
const { selected, selectedButton } = useSelection();

const button = selectedButton;
const colors = computed(() => (button.value ? categoryColors[button.value.color] : null));
const position = computed(() =>
  selected.value
    ? t("inspector.position", { row: selected.value.row + 1, col: selected.value.col + 1 })
    : "",
);

// Quick icon choices (as in the mockup); the picker has the whole set.
const QUICK = ["Mic", "Volume2", "Play", "Monitor", "Camera", "Folder", "Globe", "Moon"];
const quick = ref<Record<string, string[]>>({});
onMounted(async () => {
  const catalog = await loadIcons();
  quick.value = Object.fromEntries(
    QUICK.filter((n) => catalog[n]).map((n) => [n, iconToPaths(catalog[n]!)]),
  );
});
const pickerOpen = ref(false);

// Typing sends one edit per keystroke (the desktop merges them into one undo
// entry); leaving the field closes that entry.
function setLabel(label: string) {
  if (button.value) void edit({ type: "setLabel", params: { buttonId: button.value.id, label } });
}
function setSubtitle(subtitle: string) {
  if (button.value) {
    void edit({
      type: "setSubtitle",
      params: { buttonId: button.value.id, subtitle: subtitle || undefined },
    });
  }
}
function setIcon(icon: IconConfig | undefined) {
  pickerOpen.value = false;
  if (button.value) void edit({ type: "setIcon", params: { buttonId: button.value.id, icon } });
}
function quickIcon(name: string) {
  setIcon({ name, paths: quick.value[name] ?? [] });
}
function setColor(color: Category) {
  if (button.value) void edit({ type: "setColor", params: { buttonId: button.value.id, color } });
}
function setKeys(keys: string[]) {
  if (!button.value) return;
  const action: Action = { type: "hotkey", params: { keys } };
  void edit({ type: "setAction", params: { buttonId: button.value.id, action } });
}
async function remove() {
  if (button.value) await edit({ type: "deleteButton", params: { buttonId: button.value.id } });
}

const isHotkey = computed(() => button.value?.action.type === "hotkey");
const hotkeyKeys = computed(() =>
  button.value?.action.type === "hotkey" ? button.value.action.params.keys : [],
);
const summary = computed(() => (button.value ? actionSummary(button.value.action, t) : ""));
// The PC runs system actions itself; an empty shortcut does nothing yet.
const testable = computed(
  () =>
    !!button.value &&
    isSystemAction(button.value.action) &&
    !(isHotkey.value && hotkeyKeys.value.length === 0),
);

// "Tester l'action": runs it on this PC and shows the outcome briefly.
const test = ref<"idle" | "running" | "ok" | "error">("idle");
const testError = ref("");
let testTimer: ReturnType<typeof setTimeout> | undefined;
watch(button, (b, previous) => {
  if (b?.id !== previous?.id) test.value = "idle";
});
async function runTest() {
  if (!button.value) return;
  test.value = "running";
  clearTimeout(testTimer);
  try {
    const outcome = await testButton(button.value.id);
    if (outcome.status === "ok") {
      test.value = "ok";
    } else {
      test.value = "error";
      testError.value = t(`activity.errors.${outcome.code}`);
    }
  } catch (e) {
    test.value = "error";
    testError.value = String(e);
  }
  testTimer = setTimeout(() => (test.value = "idle"), 2000);
}

async function addShortcut() {
  const cell = selected.value;
  if (!cell) return;
  await addPreset(
    {
      id: "hotkey",
      group: "system" as never,
      color: "grey" as Category,
      icon: "Keyboard",
      action: { type: "hotkey", params: { keys: [] } },
    },
    { row: cell.row, col: cell.col },
  );
}
</script>

<template>
  <div class="flex flex-col gap-[22px]">
    <!-- Nothing selected -->
    <div v-if="!selected" class="flex flex-col gap-2.5">
      <span class="font-display text-lg font-semibold">{{ t("inspector.noneTitle") }}</span>
      <span class="text-sm leading-relaxed text-muted-2">{{ t("inspector.noneBody") }}</span>
      <span class="text-xs leading-relaxed text-muted">{{ t("editor.keyboardHint") }}</span>
    </div>

    <!-- Empty cell -->
    <div v-else-if="!button" class="flex flex-col items-start gap-2.5">
      <span class="font-display text-lg font-semibold">{{ t("inspector.emptyTitle") }}</span>
      <span class="text-sm leading-relaxed text-muted-2">{{ t("inspector.emptyBody") }}</span>
      <span class="text-[13px] text-muted">{{ position }}</span>
      <button
        type="button"
        class="mt-2 flex items-center gap-2 rounded-[10px] border border-line-2 bg-surface-2 px-3 py-2 text-sm hover:bg-surface-active"
        @click="addShortcut"
      >
        <Plus :size="15" aria-hidden="true" />
        {{ t("inspector.addShortcut") }}
      </button>
    </div>

    <!-- Button -->
    <template v-else-if="colors">
      <div class="flex items-center gap-3.5">
        <span
          class="flex size-16 shrink-0 items-center justify-center rounded-[18px] border border-line-2 bg-surface-2"
        >
          <span
            class="flex size-11 items-center justify-center rounded-xl font-display text-xl font-semibold"
            :style="{ background: colors.bg, color: colors.fg }"
          >
            <IconPaths
              v-if="button.icon"
              :paths="button.icon.paths"
              :size="22"
              :color="colors.fg"
            />
            <template v-else>{{ button.label.trim().charAt(0).toUpperCase() }}</template>
          </span>
        </span>
        <div class="flex min-w-0 flex-col gap-0.5">
          <span class="truncate font-display text-[19px] font-semibold">
            {{ button.label || t("inspector.noLabel") }}
          </span>
          <span class="text-[13px] text-muted">{{ position }}</span>
        </div>
      </div>

      <label class="field">
        <span class="field-label">{{ t("inspector.label") }}</span>
        <DraftInput
          :key="button.id"
          :value="button.label"
          :maxlength="40"
          class="field-input"
          @input="setLabel"
          @commit="checkpoint()"
        />
      </label>

      <label class="field">
        <span class="field-label">{{ t("inspector.subtitle") }}</span>
        <DraftInput
          :key="button.id"
          :value="button.subtitle ?? ''"
          :maxlength="40"
          :placeholder="t('inspector.subtitlePlaceholder')"
          class="field-input text-sm"
          @input="setSubtitle"
          @commit="checkpoint()"
        />
      </label>

      <div class="field">
        <span class="field-label">{{ t("inspector.icon") }}</span>
        <div class="grid grid-cols-5 gap-1.5">
          <button
            v-for="(paths, name) in quick"
            :key="name"
            type="button"
            class="icon-choice"
            :class="button.icon?.name === name ? 'border-accent' : 'border-line-2'"
            :aria-label="name"
            :title="name"
            @click="quickIcon(name)"
          >
            <IconPaths
              :paths="paths"
              :size="16"
              :color="button.icon?.name === name ? '#8B7CFF' : '#A3ABBD'"
            />
          </button>
          <button
            type="button"
            class="icon-choice"
            :class="!button.icon ? 'border-accent' : 'border-line-2'"
            :aria-label="t('inspector.noIcon')"
            :title="t('inspector.noIcon')"
            @click="setIcon(undefined)"
          >
            <Ban :size="16" class="text-muted-2" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="icon-choice border-line-2"
            :aria-label="t('inspector.moreIcons')"
            :title="t('inspector.moreIcons')"
            @click="pickerOpen = true"
          >
            <MoreHorizontal :size="16" class="text-muted-2" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="field">
        <span class="field-label">{{ t("inspector.color") }}</span>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="c in categories"
            :key="c"
            type="button"
            class="size-8 rounded-full border-[3px] transition-transform active:scale-90"
            :style="{
              background: categoryColors[c].fg,
              borderColor: button.color === c ? '#E8EAF0' : '#121317',
            }"
            :aria-label="t(`colors.${c}`)"
            :aria-pressed="button.color === c"
            @click="setColor(c)"
          />
        </div>
      </div>

      <div
        class="flex flex-col gap-3 rounded-[14px] border border-line-2 bg-surface-2 p-4"
        :aria-label="t('inspector.action')"
      >
        <div class="flex items-center justify-between gap-2">
          <span class="field-label">{{ t("inspector.action") }}</span>
          <span
            class="rounded-full px-2.5 py-[3px] text-xs"
            :style="{ background: colors.bg, color: colors.fg }"
          >
            {{ t(`actions.kinds.${button.action.type}`) }}
          </span>
        </div>
        <HotkeyEditor v-if="isHotkey" :keys="hotkeyKeys" @change="setKeys" />
        <template v-else>
          <p class="text-sm leading-relaxed">{{ summary }}</p>
          <p class="text-xs leading-relaxed text-muted">{{ t("actions.editLater") }}</p>
        </template>
      </div>

      <div class="flex flex-wrap gap-2.5">
        <button
          type="button"
          class="flex flex-1 items-center justify-center gap-2 rounded-[11px] px-4 py-3 text-[15px] font-semibold transition-colors"
          :class="
            test === 'error' ? 'bg-cat-red/20 text-[#FFC2C2]' : 'bg-accent text-bg hover:opacity-90'
          "
          :disabled="test === 'running' || !testable"
          :title="test === 'error' ? testError : undefined"
          @click="runTest"
        >
          <Play :size="16" aria-hidden="true" />
          {{
            test === "ok"
              ? t("inspector.tested")
              : test === "error"
                ? testError
                : t("inspector.test")
          }}
        </button>
        <button
          type="button"
          class="rounded-[11px] border border-[#3A2A2E] px-4 py-3 text-[15px] text-[#FF8A8A] transition-colors hover:bg-cat-red/10"
          @click="remove"
        >
          {{ t("common.delete") }}
        </button>
      </div>
    </template>

    <IconPicker
      v-if="pickerOpen && button && colors"
      :current="button.icon?.name ?? null"
      :color="colors.fg"
      @pick="setIcon"
      @close="pickerOpen = false"
    />
  </div>
</template>

<style scoped>
@reference "../../style.css";
.field {
  @apply flex flex-col gap-2;
}
.field-label {
  @apply text-xs font-semibold tracking-[.06em] text-muted uppercase;
}
.field-input {
  @apply rounded-[10px] border border-[#2A2D36] bg-bg px-3 py-2.5 text-[15px] outline-none focus:border-accent;
}
.icon-choice {
  @apply flex aspect-square items-center justify-center rounded-[9px] border bg-bg transition-colors hover:bg-surface-active;
}
</style>
