<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{ title: string; label: string; initial?: string; confirm: string }>();
const emit = defineEmits<{ submit: [value: string]; close: [] }>();
const { t } = useI18n();

const dialog = ref<HTMLDialogElement | null>(null);
const input = ref<HTMLInputElement | null>(null);
const value = ref(props.initial ?? "");
const maxLength = 40;

function submit() {
  const v = value.value.trim();
  if (v.length > 0) emit("submit", v);
}

onMounted(() => {
  dialog.value?.showModal();
  input.value?.select();
});
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto w-[min(26rem,calc(100vw-2rem))] rounded-2xl border border-line bg-surface p-0 text-fg backdrop:bg-black/60"
    @cancel.prevent="emit('close')"
  >
    <form class="flex flex-col gap-4 p-6" @submit.prevent="submit">
      <h2 class="font-display text-lg font-semibold">{{ title }}</h2>
      <label class="flex flex-col gap-2 text-sm">
        <span class="text-muted">{{ label }}</span>
        <input
          ref="input"
          v-model="value"
          :maxlength="maxLength"
          class="rounded-[10px] border border-[#2A2D36] bg-bg px-3 py-2.5 text-[15px] outline-none focus:border-accent"
        />
      </label>
      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="rounded-[10px] border border-line-2 px-4 py-2 text-sm hover:bg-surface-active"
          @click="emit('close')"
        >
          {{ t("common.cancel") }}
        </button>
        <button
          type="submit"
          class="rounded-[10px] bg-accent px-4 py-2 text-sm font-semibold text-bg hover:opacity-90 disabled:opacity-50"
          :disabled="value.trim().length === 0"
        >
          {{ confirm }}
        </button>
      </div>
    </form>
  </dialog>
</template>
