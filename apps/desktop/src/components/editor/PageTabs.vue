<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { AnimatePresence, motion } from "motion-v";
import {
  ChevronLeft,
  ChevronRight,
  Folder,
  MoreHorizontal,
  Pencil,
  Plus,
  Trash2,
} from "lucide-vue-next";
import TextPrompt from "../TextPrompt.vue";
import { useEditor } from "../../composables/useEditor";
import { useSelection } from "../../editor/selection";

const { t } = useI18n();
const { editedProfile, edit } = useEditor();
const { page, isFolder, showPage } = useSelection();

const pages = computed(() => editedProfile.value?.pages ?? []);
const folders = computed(() => editedProfile.value?.folders ?? []);
const index = computed(() => pages.value.findIndex((p) => p.id === page.value?.id));
const menuOpen = ref(false);
const confirmDelete = ref(false);
const menu = ref<HTMLElement | null>(null);
onClickOutside(menu, () => closeMenu());
onKeyStroke("Escape", () => menuOpen.value && closeMenu());
const prompt = ref<"add" | "rename" | null>(null);

function closeMenu() {
  menuOpen.value = false;
  confirmDelete.value = false;
}

async function submit(name: string) {
  const kind = prompt.value;
  prompt.value = null;
  if (kind === "add" && editedProfile.value) {
    const outcome = await edit({
      type: "addPage",
      params: { profileId: editedProfile.value.id, name },
    });
    if (outcome?.createdId) showPage(outcome.createdId);
  } else if (kind === "rename" && page.value) {
    await edit({ type: "renamePage", params: { pageId: page.value.id, name } });
  }
}

async function move(delta: number) {
  if (!page.value) return;
  await edit({
    type: "movePage",
    params: { pageId: page.value.id, index: Math.max(0, index.value + delta) },
  });
}

async function remove() {
  if (!page.value) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    return;
  }
  closeMenu();
  await edit({ type: "deletePage", params: { pageId: page.value.id } });
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-1.5" role="tablist" :aria-label="t('pages.label')">
    <button
      v-for="p in pages"
      :key="p.id"
      type="button"
      role="tab"
      :aria-selected="p.id === page?.id"
      class="rounded-[9px] border px-3.5 py-[7px] text-sm transition-colors"
      :class="
        p.id === page?.id
          ? 'border-[#34374A] bg-[#22242C] font-semibold text-fg'
          : 'border-line-2 text-muted-2 hover:bg-surface-active'
      "
      @click="showPage(p.id)"
      @dblclick="prompt = 'rename'"
    >
      {{ p.name }}
    </button>
    <template v-if="folders.length > 0">
      <span class="mx-1 h-5 w-px bg-line-2" aria-hidden="true" />
      <button
        v-for="f in folders"
        :key="f.id"
        type="button"
        role="tab"
        :aria-selected="f.id === page?.id"
        :title="t('folders.tab', { name: f.name })"
        class="flex items-center gap-1.5 rounded-[9px] border px-3 py-[7px] text-sm transition-colors"
        :class="
          f.id === page?.id
            ? 'border-[#34374A] bg-[#22242C] font-semibold text-fg'
            : 'border-line-2 text-muted-2 hover:bg-surface-active'
        "
        @click="showPage(f.id)"
        @dblclick="prompt = 'rename'"
      >
        <Folder :size="14" aria-hidden="true" />
        {{ f.name }}
      </button>
    </template>
    <div ref="menu" class="relative">
      <button
        type="button"
        class="flex h-[34px] w-8 items-center justify-center rounded-[9px] border border-line-2 text-muted-2 hover:bg-surface-active"
        :aria-label="t('pages.menu')"
        :aria-expanded="menuOpen"
        @click="menuOpen ? closeMenu() : (menuOpen = true)"
      >
        <MoreHorizontal :size="15" aria-hidden="true" />
      </button>
      <AnimatePresence>
        <motion.div
          v-if="menuOpen"
          role="menu"
          class="absolute top-full left-0 z-20 mt-2 flex w-56 flex-col gap-1 rounded-xl border border-line-2 bg-surface-2 p-1.5 shadow-2xl shadow-black/50"
          :initial="{ opacity: 0, y: -4 }"
          :animate="{ opacity: 1, y: 0 }"
          :exit="{ opacity: 0, y: -4 }"
          :transition="{ duration: 0.12 }"
        >
          <button
            type="button"
            class="menu-item"
            @click="
              closeMenu();
              prompt = 'rename';
            "
          >
            <Pencil :size="15" aria-hidden="true" />
            {{ t(isFolder ? "pages.renameFolder" : "pages.rename") }}
          </button>
          <template v-if="!isFolder">
            <button type="button" class="menu-item" :disabled="index <= 0" @click="move(-1)">
              <ChevronLeft :size="15" aria-hidden="true" />{{ t("pages.moveLeft") }}
            </button>
            <button
              type="button"
              class="menu-item"
              :disabled="index >= pages.length - 1"
              @click="move(1)"
            >
              <ChevronRight :size="15" aria-hidden="true" />{{ t("pages.moveRight") }}
            </button>
          </template>
          <button
            type="button"
            class="menu-item"
            :class="confirmDelete ? 'bg-cat-red/15 text-[#FFC2C2]' : 'text-[#FF8A8A]'"
            :disabled="!isFolder && pages.length <= 1"
            @click="remove"
          >
            <Trash2 :size="15" aria-hidden="true" />
            {{
              confirmDelete
                ? t("pages.confirmDelete")
                : t(isFolder ? "pages.deleteFolder" : "pages.delete")
            }}
          </button>
        </motion.div>
      </AnimatePresence>
    </div>
    <button
      type="button"
      class="flex h-[34px] w-[34px] items-center justify-center rounded-[9px] border border-dashed border-[#34374A] text-muted-2 hover:bg-surface-active"
      :aria-label="t('pages.add')"
      :title="t('pages.add')"
      @click="prompt = 'add'"
    >
      <Plus :size="14" aria-hidden="true" />
    </button>
    <TextPrompt
      v-if="prompt"
      :title="t(prompt === 'add' ? 'pages.add' : isFolder ? 'pages.renameFolder' : 'pages.rename')"
      :label="t(isFolder && prompt === 'rename' ? 'pages.folderNameLabel' : 'pages.nameLabel')"
      :initial="prompt === 'rename' ? page?.name : t('pages.defaultName', { n: pages.length + 1 })"
      :confirm="t(prompt === 'add' ? 'common.create' : 'common.rename')"
      @submit="submit"
      @close="prompt = null"
    />
  </div>
</template>

<style scoped>
@reference "../../style.css";
.menu-item {
  @apply flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm hover:bg-surface-active disabled:pointer-events-none disabled:opacity-40;
}
</style>
