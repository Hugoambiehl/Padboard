<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { open as pickPath } from "@tauri-apps/plugin-dialog";
import { ChevronDown, ChevronUp, FileText, FolderOpen, Plus, X } from "lucide-vue-next";
import HotkeyEditor from "./HotkeyEditor.vue";
import DraftInput from "./DraftInput.vue";
import { isWindows } from "../../editor/keys";
import { useEditor } from "../../composables/useEditor";
import { useObs } from "../../composables/useObs";
import { defaultAction, ACTION_KINDS, type ActionKind } from "../../editor/actionDefaults";
import type { Action, MultiStep } from "../../generated/types";

// Edits one action; recursive for the steps of a multi action (which cannot
// contain multi actions). `commit` tells the parent to close the undo entry
// (a choice in a list), as opposed to typing (merged into one entry).
const props = defineProps<{ action: Action; allowMulti?: boolean; testid?: string }>();
const emit = defineEmits<{ change: [action: Action, commit: boolean] }>();
const { t } = useI18n();
const { editedProfile } = useEditor();
const { info: obs } = useObs();

const kinds = computed(() => ACTION_KINDS.filter((k) => props.allowMulti || k !== "multi"));

function change(action: Action, commit = true) {
  emit("change", action, commit);
}

function setKind(kind: ActionKind) {
  if (kind !== props.action.type) change(defaultAction(kind));
}

/** Changes some params of the current action. */
function params(patch: Record<string, unknown>, commit = true) {
  const current = props.action;
  if (current.type === "lock") return;
  change({ type: current.type, params: { ...current.params, ...patch } } as Action, commit);
}

const pages = computed(() => editedProfile.value?.pages ?? []);
const folders = computed(() => editedProfile.value?.folders ?? []);

const obsLists = computed(() => {
  const i = obs.value;
  return i?.available && i.status.state === "connected"
    ? { scenes: i.scenes, inputs: i.inputs }
    : null;
});

async function browse(directory: boolean) {
  const path = await pickPath({
    title: t(directory ? "actions.form.pickFolderTitle" : "actions.form.pickFileTitle"),
    directory,
    multiple: false,
  });
  if (typeof path === "string") params({ target: path });
}

// Multi action steps.
const steps = computed<MultiStep[]>(() =>
  props.action.type === "multi" ? props.action.params.steps : [],
);
function setSteps(next: MultiStep[], commit = true) {
  change({ type: "multi", params: { steps: next } }, commit);
}
function setStep(index: number, patch: Partial<MultiStep>, commit = true) {
  setSteps(
    steps.value.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    commit,
  );
}
function moveStep(index: number, delta: number) {
  const next = [...steps.value];
  const [step] = next.splice(index, 1);
  next.splice(index + delta, 0, step!);
  setSteps(next);
}
function delay(event: Event): number {
  const value = Math.round(Number((event.target as HTMLInputElement).value));
  return Number.isFinite(value) ? Math.min(Math.max(value, 0), 10000) : 0;
}
</script>

<template>
  <div class="flex flex-col gap-3" :data-testid="testid">
    <label class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("actions.form.kind") }}
      <select
        class="control"
        :value="action.type"
        data-field="kind"
        @change="setKind(($event.target as HTMLSelectElement).value as ActionKind)"
      >
        <option v-for="k in kinds" :key="k" :value="k">{{ t(`actions.kinds.${k}`) }}</option>
      </select>
    </label>

    <HotkeyEditor
      v-if="action.type === 'hotkey'"
      :keys="action.params.keys"
      @change="(keys) => params({ keys })"
    />

    <label v-else-if="action.type === 'media'" class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("actions.form.command") }}
      <select
        class="control"
        :value="action.params.command"
        data-field="command"
        @change="params({ command: ($event.target as HTMLSelectElement).value })"
      >
        <option v-for="c in ['play_pause', 'next', 'previous', 'stop']" :key="c" :value="c">
          {{ t(`actions.form.mediaCommands.${c}`) }}
        </option>
      </select>
    </label>

    <template v-else-if="action.type === 'volume'">
      <label class="flex flex-col gap-1.5 text-xs text-muted">
        {{ t("actions.form.command") }}
        <select
          class="control"
          :value="action.params.command"
          data-field="command"
          @change="
            params({
              command: ($event.target as HTMLSelectElement).value,
              value: ($event.target as HTMLSelectElement).value === 'set' ? 50 : undefined,
            })
          "
        >
          <option v-for="c in ['up', 'down', 'set', 'toggle_mute']" :key="c" :value="c">
            {{ t(`actions.form.volumeCommands.${c}`) }}
          </option>
        </select>
      </label>
      <label
        v-if="action.params.command !== 'toggle_mute'"
        class="flex flex-col gap-1.5 text-xs text-muted"
      >
        {{ t(action.params.command === "set" ? "actions.form.level" : "actions.form.step") }}
        <input
          class="control"
          type="number"
          data-field="value"
          :min="action.params.command === 'set' ? 0 : 1"
          max="100"
          :value="action.params.value ?? 5"
          @change="
            params({
              value: Math.min(
                100,
                Math.max(
                  action.params.command === 'set' ? 0 : 1,
                  Math.round(Number(($event.target as HTMLInputElement).value)) || 0,
                ),
              ),
            })
          "
        />
      </label>
    </template>

    <label
      v-else-if="action.type === 'microphone'"
      class="flex flex-col gap-1.5 text-xs text-muted"
    >
      {{ t("actions.form.command") }}
      <select
        class="control"
        :value="action.params.command"
        data-field="command"
        @change="params({ command: ($event.target as HTMLSelectElement).value })"
      >
        <option v-for="c in ['toggle_mute', 'mute', 'unmute']" :key="c" :value="c">
          {{ t(`actions.form.micCommands.${c}`) }}
        </option>
      </select>
    </label>

    <template v-else-if="action.type === 'open'">
      <label class="flex flex-col gap-1.5 text-xs text-muted">
        {{ t("actions.form.target") }}
        <DraftInput
          class="control"
          data-field="target"
          :value="action.params.target"
          :maxlength="2048"
          :placeholder="
            t(isWindows ? 'actions.form.targetPlaceholderWin' : 'actions.form.targetPlaceholder')
          "
          @input="(target) => params({ target }, false)"
        />
      </label>
      <div class="flex gap-2">
        <button type="button" class="small" @click="browse(false)">
          <FileText :size="14" aria-hidden="true" />{{ t("actions.form.pickFile") }}
        </button>
        <button type="button" class="small" @click="browse(true)">
          <FolderOpen :size="14" aria-hidden="true" />{{ t("actions.form.pickFolder") }}
        </button>
      </div>
    </template>

    <p v-else-if="action.type === 'lock'" class="text-sm text-muted-2">
      {{ t("actions.form.lockHint") }}
    </p>

    <template v-else-if="action.type === 'navigate'">
      <label class="flex flex-col gap-1.5 text-xs text-muted">
        {{ t("actions.form.command") }}
        <select
          class="control"
          :value="action.params.to"
          data-field="to"
          @change="params({ to: ($event.target as HTMLSelectElement).value, id: undefined })"
        >
          <option
            v-for="to in ['page', 'folder', 'next', 'previous', 'back']"
            :key="to"
            :value="to"
          >
            {{ t(`actions.form.navTo.${to}`) }}
          </option>
        </select>
      </label>
      <label
        v-if="action.params.to === 'page' || action.params.to === 'folder'"
        class="flex flex-col gap-1.5 text-xs text-muted"
      >
        {{ t(action.params.to === "page" ? "actions.form.page" : "actions.form.folder") }}
        <select
          class="control"
          :value="action.params.id ?? ''"
          data-field="id"
          @change="params({ id: ($event.target as HTMLSelectElement).value || undefined })"
        >
          <option value="" disabled>{{ t("actions.form.choose") }}</option>
          <option
            v-for="g in action.params.to === 'page' ? pages : folders"
            :key="g.id"
            :value="g.id"
          >
            {{ g.name }}
          </option>
        </select>
      </label>
    </template>

    <template v-else-if="action.type === 'obs'">
      <label class="flex flex-col gap-1.5 text-xs text-muted">
        {{ t("actions.form.command") }}
        <select
          class="control"
          :value="action.params.command"
          data-field="command"
          @change="
            params({ command: ($event.target as HTMLSelectElement).value, target: undefined })
          "
        >
          <option
            v-for="c in ['set_scene', 'toggle_record', 'toggle_stream', 'toggle_input_mute']"
            :key="c"
            :value="c"
          >
            {{ t(`actions.form.obsCommands.${c}`) }}
          </option>
        </select>
      </label>
      <template
        v-if="
          action.params.command === 'set_scene' || action.params.command === 'toggle_input_mute'
        "
      >
        <label class="flex flex-col gap-1.5 text-xs text-muted">
          {{
            t(action.params.command === "set_scene" ? "actions.form.scene" : "actions.form.input")
          }}
          <select
            v-if="obsLists"
            class="control"
            :value="action.params.target ?? ''"
            data-field="target"
            @change="params({ target: ($event.target as HTMLSelectElement).value || undefined })"
          >
            <option value="" disabled>{{ t("actions.form.choose") }}</option>
            <option
              v-for="name in action.params.command === 'set_scene'
                ? obsLists.scenes
                : obsLists.inputs"
              :key="name"
              :value="name"
            >
              {{ name }}
            </option>
            <option
              v-if="
                action.params.target &&
                !(
                  action.params.command === 'set_scene' ? obsLists.scenes : obsLists.inputs
                ).includes(action.params.target)
              "
              :value="action.params.target"
            >
              {{ t("actions.form.missing", { name: action.params.target }) }}
            </option>
          </select>
          <DraftInput
            v-else
            class="control"
            data-field="target"
            :value="action.params.target ?? ''"
            :maxlength="256"
            @input="(target) => params({ target: target || undefined }, false)"
          />
        </label>
        <p v-if="!obsLists" class="text-xs leading-relaxed text-muted">
          {{ t("actions.form.obsOffline") }}
        </p>
      </template>
    </template>

    <div v-else-if="action.type === 'multi'" class="flex flex-col gap-2.5">
      <span class="text-xs font-semibold tracking-[.06em] text-muted uppercase">
        {{ t("actions.form.steps") }}
      </span>
      <p v-if="steps.length === 0" class="text-sm text-muted">{{ t("actions.form.noStep") }}</p>
      <div
        v-for="(step, i) in steps"
        :key="i"
        class="flex flex-col gap-2.5 rounded-xl border border-line-2 bg-bg p-3"
        :data-step="i"
      >
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold">{{ t("actions.form.stepN", { n: i + 1 }) }}</span>
          <span class="flex gap-1">
            <button
              type="button"
              class="icon"
              :aria-label="t('actions.form.moveUp', { n: i + 1 })"
              :disabled="i === 0"
              @click="moveStep(i, -1)"
            >
              <ChevronUp :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="icon"
              :aria-label="t('actions.form.moveDown', { n: i + 1 })"
              :disabled="i === steps.length - 1"
              @click="moveStep(i, 1)"
            >
              <ChevronDown :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="icon"
              :aria-label="t('actions.form.removeStep', { n: i + 1 })"
              @click="setSteps(steps.filter((_, j) => j !== i))"
            >
              <X :size="14" aria-hidden="true" />
            </button>
          </span>
        </div>
        <ActionForm
          :action="step.action"
          @change="(action, commit) => setStep(i, { action }, commit)"
        />
        <label class="flex items-center justify-between gap-2 text-xs text-muted">
          {{ t("actions.form.delay") }}
          <input
            class="control w-24"
            type="number"
            min="0"
            max="10000"
            step="50"
            data-field="delay"
            :value="step.delayMs ?? 0"
            @change="setStep(i, { delayMs: delay($event) })"
          />
        </label>
      </div>
      <button
        type="button"
        class="small self-start"
        :disabled="steps.length >= 20"
        @click="setSteps([...steps, { action: defaultAction('hotkey'), delayMs: 0 }])"
      >
        <Plus :size="14" aria-hidden="true" />{{ t("actions.form.addStep") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
@reference "../../style.css";
.control {
  @apply rounded-[10px] border border-line-2 bg-bg px-3 py-2 text-sm text-fg outline-none focus:border-accent;
}
.small {
  @apply flex items-center gap-1.5 rounded-[10px] border border-line-2 bg-surface-2 px-3 py-1.5 text-sm text-fg hover:bg-surface-active disabled:opacity-40;
}
.icon {
  @apply flex size-7 items-center justify-center rounded-lg border border-line-2 text-muted-2 hover:bg-surface-active disabled:pointer-events-none disabled:opacity-35;
}
</style>
