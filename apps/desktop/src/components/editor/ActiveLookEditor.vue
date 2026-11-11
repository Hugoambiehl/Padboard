<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { RotateCcw } from "lucide-vue-next";
import DraftInput from "./DraftInput.vue";
import IconPaths from "./IconPaths.vue";
import IconPicker from "./IconPicker.vue";
import { categories, categoryColors } from "../../editor/colors";
import type { ActiveLook, ButtonConfig, Category, IconConfig } from "../../generated/types";

// The look of a stateful button while active (microphone muted, recording,
// current scene…). Each field left on "Auto" keeps the automatic look.
const props = defineProps<{ button: ButtonConfig }>();
const emit = defineEmits<{ change: [look: ActiveLook | undefined, commit: boolean]; commit: [] }>();
const { t } = useI18n();
const picking = ref(false);

function update(patch: Partial<ActiveLook>, commit = true) {
  const look: ActiveLook = { ...(props.button.activeLook ?? {}), ...patch };
  for (const key of Object.keys(look) as (keyof ActiveLook)[]) {
    if (look[key] === undefined || look[key] === "") delete look[key];
  }
  emit("change", Object.keys(look).length > 0 ? look : undefined, commit);
}

function pickIcon(icon: IconConfig | undefined) {
  picking.value = false;
  update({ icon });
}

const look = () => props.button.activeLook ?? {};
</script>

<template>
  <div
    class="flex flex-col gap-3 rounded-[14px] border border-line-2 bg-surface-2 p-4"
    data-testid="active-look"
  >
    <span class="text-xs font-semibold tracking-[.06em] text-muted uppercase">
      {{ t("inspector.activeLook.title") }}
    </span>
    <p class="text-xs leading-relaxed text-muted">{{ t("inspector.activeLook.hint") }}</p>

    <label class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("inspector.activeLook.label") }}
      <DraftInput
        :key="button.id"
        class="rounded-[10px] border border-line-2 bg-bg px-3 py-2 text-sm text-fg outline-none focus:border-accent"
        data-field="active-label"
        :value="look().label ?? ''"
        :maxlength="40"
        :placeholder="t('inspector.activeLook.labelPlaceholder')"
        @input="(label) => update({ label }, false)"
        @commit="emit('commit')"
      />
    </label>

    <div class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("inspector.activeLook.color") }}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="rounded-full border px-2.5 py-1 text-xs"
          :class="!look().color ? 'border-fg text-fg' : 'border-line-2 text-muted-2'"
          :aria-pressed="!look().color"
          @click="update({ color: undefined })"
        >
          {{ t("inspector.activeLook.auto") }}
        </button>
        <button
          v-for="c in categories"
          :key="c"
          type="button"
          class="size-7 rounded-full border-[3px]"
          :style="{
            background: categoryColors[c].fg,
            borderColor: look().color === c ? '#E8EAF0' : '#17181D',
          }"
          :aria-label="`${t('inspector.activeLook.color')} ${t(`colors.${c}`)}`"
          :aria-pressed="look().color === c"
          @click="update({ color: c as Category })"
        />
      </div>
    </div>

    <div class="flex flex-col gap-1.5 text-xs text-muted">
      {{ t("inspector.activeLook.icon") }}
      <div class="flex items-center gap-2">
        <button
          type="button"
          class="rounded-full border px-2.5 py-1 text-xs"
          :class="!look().icon ? 'border-fg text-fg' : 'border-line-2 text-muted-2'"
          :aria-pressed="!look().icon"
          @click="update({ icon: undefined })"
        >
          {{ t("inspector.activeLook.auto") }}
        </button>
        <span
          v-if="look().icon"
          class="flex size-8 items-center justify-center rounded-lg border border-accent bg-bg"
        >
          <IconPaths :paths="look().icon!.paths" :size="16" color="#8B7CFF" />
        </span>
        <button
          type="button"
          class="rounded-full border border-line-2 px-2.5 py-1 text-xs text-muted-2 hover:bg-surface-active"
          @click="picking = true"
        >
          {{ t("actions.form.choose") }}
        </button>
      </div>
    </div>

    <button
      v-if="button.activeLook"
      type="button"
      class="flex items-center gap-1.5 self-start text-xs text-muted-2 hover:text-fg"
      @click="emit('change', undefined, true)"
    >
      <RotateCcw :size="12" aria-hidden="true" />{{ t("inspector.activeLook.reset") }}
    </button>

    <IconPicker
      v-if="picking"
      :current="look().icon?.name ?? null"
      :color="categoryColors[look().color ?? button.color].fg"
      @pick="pickIcon"
      @close="picking = false"
    />
  </div>
</template>
