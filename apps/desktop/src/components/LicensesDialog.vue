<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Scale, Search, X } from "lucide-vue-next";
import { filterPackages, loadLicenses, type Licenses } from "../licenses";

const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();

const dialog = ref<HTMLDialogElement | null>(null);
const licenses = ref<Licenses | null>(null);
const loaded = ref(false);
const query = ref("");
/** Expanded rows ("project" or "<ecosystem>:<name>@<version>"): texts are only rendered when open. */
const open = reactive(new Set<string>());

const shown = computed(() =>
  licenses.value ? filterPackages(licenses.value.packages, query.value) : [],
);

function toggle(key: string) {
  if (open.has(key)) open.delete(key);
  else open.add(key);
}

onMounted(async () => {
  dialog.value?.showModal();
  try {
    licenses.value = await loadLicenses();
  } finally {
    loaded.value = true;
  }
});
</script>

<template>
  <dialog
    ref="dialog"
    class="m-auto h-[min(44rem,calc(100vh-2rem))] w-[min(46rem,calc(100vw-2rem))] flex-col rounded-2xl open:flex border border-line bg-surface p-0 text-fg backdrop:bg-black/60"
    aria-labelledby="licenses-title"
    @cancel.prevent="emit('close')"
  >
    <header class="flex items-start gap-3 border-b border-line p-5">
      <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-accent/15">
        <Scale :size="18" class="text-accent" aria-hidden="true" />
      </span>
      <div class="flex min-w-0 flex-1 flex-col gap-1">
        <h2 id="licenses-title" class="font-display text-lg font-semibold">
          {{ t("licenses.title") }}
        </h2>
        <p class="text-sm text-muted">{{ t("licenses.intro") }}</p>
      </div>
      <button
        type="button"
        class="rounded-lg p-1.5 text-muted transition-colors hover:bg-surface-active hover:text-fg"
        :aria-label="t('licenses.close')"
        @click="emit('close')"
      >
        <X :size="18" aria-hidden="true" />
      </button>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-5">
      <p v-if="loaded && !licenses" class="text-sm text-muted-2">{{ t("licenses.missing") }}</p>
      <template v-if="licenses">
        <!-- Padboard's own license first. -->
        <div class="rounded-xl border border-line-2 bg-surface-2">
          <button
            type="button"
            class="flex w-full items-center gap-3 px-3 py-2.5 text-left"
            :aria-expanded="open.has('project')"
            @click="toggle('project')"
          >
            <span class="flex-1 text-sm font-semibold">{{ t("app.name") }}</span>
            <span class="text-xs text-muted-2">{{ licenses.project.license }}</span>
            <ChevronDown
              :size="16"
              class="text-muted transition-transform"
              :class="{ 'rotate-180': open.has('project') }"
              aria-hidden="true"
            />
          </button>
          <pre
            v-if="open.has('project')"
            class="max-h-72 overflow-auto border-t border-line-2 px-3 py-2 font-mono text-xs whitespace-pre-wrap text-muted-2"
            >{{ licenses.project.text }}</pre>
        </div>

        <label class="relative flex items-center">
          <Search
            :size="16"
            class="pointer-events-none absolute left-3 text-muted"
            aria-hidden="true"
          />
          <input
            v-model="query"
            type="search"
            class="control w-full pl-9"
            :placeholder="t('licenses.search')"
            :aria-label="t('licenses.search')"
            data-licenses-search
          />
        </label>
        <p class="text-xs text-muted">
          {{ t("licenses.count", { n: shown.length, total: licenses.packages.length }) }}
        </p>

        <ul class="flex flex-col gap-1" data-licenses-list>
          <li
            v-for="p in shown"
            :key="`${p.ecosystem}:${p.name}@${p.version}`"
            class="rounded-xl border border-line-2"
          >
            <button
              type="button"
              class="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-surface-active"
              :aria-expanded="open.has(`${p.ecosystem}:${p.name}@${p.version}`)"
              @click="toggle(`${p.ecosystem}:${p.name}@${p.version}`)"
            >
              <span class="min-w-0 flex-1 truncate text-sm">
                {{ p.name }} <span class="text-muted">{{ p.version }}</span>
              </span>
              <span class="rounded-md bg-surface-2 px-1.5 py-0.5 text-[11px] text-muted">
                {{ t(`licenses.ecosystem.${p.ecosystem}`) }}
              </span>
              <span class="max-w-[40%] truncate text-xs text-muted-2">{{ p.license }}</span>
              <ChevronDown
                :size="16"
                class="shrink-0 text-muted transition-transform"
                :class="{ 'rotate-180': open.has(`${p.ecosystem}:${p.name}@${p.version}`) }"
                aria-hidden="true"
              />
            </button>
            <div
              v-if="open.has(`${p.ecosystem}:${p.name}@${p.version}`)"
              class="flex flex-col gap-2 border-t border-line-2 px-3 py-2"
            >
              <p v-if="p.url" class="truncate font-mono text-xs text-muted select-all">
                {{ p.url }}
              </p>
              <p v-if="p.texts.length === 0" class="text-xs text-muted-2">
                {{ t("licenses.noText", { license: p.license }) }}
              </p>
              <pre
                v-for="i in p.texts"
                :key="i"
                class="max-h-72 overflow-auto rounded-lg bg-bg px-3 py-2 font-mono text-xs whitespace-pre-wrap text-muted-2"
                >{{ licenses.texts[i] }}</pre>
            </div>
          </li>
        </ul>
      </template>
    </div>
  </dialog>
</template>
