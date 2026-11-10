<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Search, X } from "lucide-vue-next";
import IconPaths from "./IconPaths.vue";
import { loadIcons } from "../../icons/catalog";
import { iconToPaths, type IconNode } from "../../icons/paths";
import type { IconConfig } from "../../generated/types";

const props = defineProps<{ current: string | null; color: string }>();
const emit = defineEmits<{ pick: [icon: IconConfig]; close: [] }>();
const { t } = useI18n();

/** Shown at once; the search narrows the ~2,300 icons. */
const LIMIT = 240;

const dialog = ref<HTMLDialogElement | null>(null);
const query = ref("");
const catalog = ref<Record<string, IconNode>>({});

onMounted(async () => {
  dialog.value?.showModal();
  catalog.value = await loadIcons();
});

/** "VolumeX" → "volume x", so "volume" or "volumex" both match. */
const words = (name: string) => name.replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();

const matches = computed(() => {
  const q = query.value.trim().toLowerCase();
  const names = Object.keys(catalog.value);
  const found = q
    ? names.filter((n) => words(n).includes(q) || n.toLowerCase().includes(q.replace(/\s+/g, "")))
    : names;
  return { total: found.length, shown: found.slice(0, LIMIT) };
});

const paths = (name: string) => iconToPaths(catalog.value[name]!);

function pick(name: string) {
  emit("pick", { name, paths: paths(name) });
}
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto flex h-[min(38rem,calc(100vh-4rem))] w-[min(40rem,calc(100vw-2rem))] flex-col rounded-2xl border border-line bg-surface p-0 text-fg backdrop:bg-black/60"
    :aria-label="t('icons.title')"
    @cancel.prevent="emit('close')"
  >
    <header class="flex items-center gap-3 border-b border-line p-4">
      <label
        class="flex flex-1 items-center gap-2 rounded-[10px] border border-line-2 bg-bg px-3 focus-within:border-accent"
      >
        <Search :size="16" class="text-muted" aria-hidden="true" />
        <span class="sr-only">{{ t("icons.search") }}</span>
        <input
          v-model="query"
          autofocus
          :placeholder="t('icons.searchPlaceholder')"
          class="min-w-0 flex-1 bg-transparent py-2.5 text-sm outline-none"
        />
      </label>
      <button
        type="button"
        class="rounded-lg p-2 text-muted hover:bg-surface-active hover:text-fg"
        :aria-label="t('common.close')"
        @click="emit('close')"
      >
        <X :size="18" aria-hidden="true" />
      </button>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <p v-if="Object.keys(catalog).length === 0" class="text-sm text-muted">
        {{ t("app.loading") }}
      </p>
      <div v-else class="grid grid-cols-[repeat(auto-fill,minmax(3rem,1fr))] gap-1.5">
        <button
          v-for="name in matches.shown"
          :key="name"
          type="button"
          class="flex aspect-square items-center justify-center rounded-[9px] border bg-bg transition-colors hover:bg-surface-active"
          :class="name === props.current ? 'border-accent' : 'border-line-2'"
          :title="name"
          :aria-label="name"
          @click="pick(name)"
        >
          <IconPaths
            :paths="paths(name)"
            :size="20"
            :color="name === props.current ? color : '#A3ABBD'"
          />
        </button>
      </div>
      <p v-if="matches.total > matches.shown.length" class="mt-3 text-xs text-muted">
        {{ t("icons.more", { count: matches.total - matches.shown.length }) }}
      </p>
      <p
        v-else-if="matches.total === 0 && Object.keys(catalog).length > 0"
        class="text-sm text-muted"
      >
        {{ t("icons.none") }}
      </p>
    </div>
    <footer class="border-t border-line px-4 py-2 text-[11px] text-muted">
      {{ t("icons.credit") }}
    </footer>
  </dialog>
</template>
