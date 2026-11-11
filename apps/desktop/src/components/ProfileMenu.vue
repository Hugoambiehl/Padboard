<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { AnimatePresence, motion } from "motion-v";
import { Check, ChevronDown, Copy, MonitorUp, Pencil, Plus, Tablet, Trash2 } from "lucide-vue-next";
import TextPrompt from "./TextPrompt.vue";
import { useEditor } from "../composables/useEditor";

const { t } = useI18n();
const { config, editedProfile, displayedProfile, editingDisplayed, editProfile, display, edit } =
  useEditor();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
onClickOutside(root, () => close());
onKeyStroke("Escape", () => open.value && close());

type Prompt = "create" | "duplicate" | "rename" | null;
const prompt = ref<Prompt>(null);
const confirmDelete = ref(false);
const profiles = computed(() => config.value?.profiles ?? []);

function close() {
  open.value = false;
  confirmDelete.value = false;
}

// Choosing a profile only opens it in the editor; the tablets keep theirs.
function choose(profileId: string) {
  close();
  editProfile(profileId);
}

async function submit(name: string) {
  const current = editedProfile.value;
  const kind = prompt.value;
  prompt.value = null;
  if (kind === "create") {
    const outcome = await edit({ type: "addProfile", params: { name } });
    if (outcome?.createdId) choose(outcome.createdId);
  } else if (kind === "duplicate" && current) {
    const outcome = await edit({
      type: "duplicateProfile",
      params: { profileId: current.id, name },
    });
    if (outcome?.createdId) choose(outcome.createdId);
  } else if (kind === "rename" && current) {
    await edit({ type: "renameProfile", params: { profileId: current.id, name } });
  }
}

async function remove() {
  const current = editedProfile.value;
  if (!current) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    return;
  }
  close();
  await edit({ type: "deleteProfile", params: { profileId: current.id } });
}

function ask(kind: Prompt) {
  close();
  prompt.value = kind;
}

// Deleting the displayed profile moves the tablets to the first remaining
// one (the desktop decides; this only tells the user beforehand).
const deleteConfirm = computed(() => {
  const next = editingDisplayed.value
    ? profiles.value.find((p) => p.id !== editedProfile.value?.id)
    : undefined;
  return next
    ? t("profiles.delete.confirmDisplayed", { name: next.name })
    : t("profiles.delete.confirm");
});

const promptTitle = computed(() => (prompt.value ? t(`profiles.${prompt.value}.title`) : ""));
const promptInitial = computed(() => {
  const name = editedProfile.value?.name ?? "";
  if (prompt.value === "rename") return name;
  if (prompt.value === "duplicate") return t("profiles.duplicate.defaultName", { name });
  return "";
});
</script>

<template>
  <div class="flex items-center gap-2">
    <div ref="root" class="relative">
      <button
        type="button"
        class="flex items-center gap-2 rounded-[10px] border border-line-2 bg-surface-2 px-3 py-[7px] text-sm transition-colors hover:bg-surface-active"
        :aria-expanded="open"
        aria-haspopup="menu"
        @click="open ? close() : (open = true)"
      >
        {{ t("profiles.label") }}
        <strong class="max-w-48 truncate font-semibold">{{ editedProfile?.name }}</strong>
        <span
          v-if="editingDisplayed"
          class="text-cat-teal"
          role="img"
          :title="t('profiles.displayed')"
          :aria-label="t('profiles.displayed')"
        >
          <Tablet :size="14" aria-hidden="true" />
        </span>
        <ChevronDown :size="14" class="text-muted" aria-hidden="true" />
      </button>

      <AnimatePresence>
        <motion.div
          v-if="open"
          role="menu"
          class="absolute top-full left-0 z-20 mt-2 flex w-64 flex-col gap-1 rounded-xl border border-line-2 bg-surface-2 p-1.5 shadow-2xl shadow-black/50"
          :initial="{ opacity: 0, y: -4 }"
          :animate="{ opacity: 1, y: 0 }"
          :exit="{ opacity: 0, y: -4 }"
          :transition="{ duration: 0.12 }"
        >
          <button
            v-for="profile in profiles"
            :key="profile.id"
            type="button"
            role="menuitemradio"
            :aria-checked="profile.id === editedProfile?.id"
            class="flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm hover:bg-surface-active"
            @click="choose(profile.id)"
          >
            <Check
              :size="15"
              class="shrink-0"
              :class="profile.id === editedProfile?.id ? 'text-accent' : 'invisible'"
              aria-hidden="true"
            />
            <span class="flex-1 truncate">{{ profile.name }}</span>
            <span
              v-if="profile.id === displayedProfile?.id"
              class="shrink-0 text-cat-teal"
              role="img"
              :title="t('profiles.displayed')"
              :aria-label="t('profiles.displayed')"
            >
              <Tablet :size="15" aria-hidden="true" />
            </span>
          </button>
          <div class="my-1 h-px bg-line" />
          <button type="button" role="menuitem" class="menu-item" @click="ask('create')">
            <Plus :size="15" aria-hidden="true" />{{ t("profiles.create.action") }}
          </button>
          <button type="button" role="menuitem" class="menu-item" @click="ask('duplicate')">
            <Copy :size="15" aria-hidden="true" />{{ t("profiles.duplicate.action") }}
          </button>
          <button type="button" role="menuitem" class="menu-item" @click="ask('rename')">
            <Pencil :size="15" aria-hidden="true" />{{ t("profiles.rename.action") }}
          </button>
          <button
            type="button"
            role="menuitem"
            class="menu-item"
            :class="confirmDelete ? 'bg-cat-red/15 text-[#FFC2C2]' : 'text-[#FF8A8A]'"
            :disabled="profiles.length <= 1"
            @click="remove"
          >
            <Trash2 :size="15" aria-hidden="true" />
            {{ confirmDelete ? deleteConfirm : t("profiles.delete.action") }}
          </button>
        </motion.div>
      </AnimatePresence>

      <TextPrompt
        v-if="prompt"
        :title="promptTitle"
        :label="t('profiles.nameLabel')"
        :initial="promptInitial"
        :confirm="t(prompt === 'rename' ? 'common.rename' : 'common.create')"
        @submit="submit"
        @close="prompt = null"
      />
    </div>
    <button
      v-if="!editingDisplayed && editedProfile"
      type="button"
      class="flex items-center gap-2 rounded-[10px] border border-cat-teal/35 bg-cat-teal/10 px-3 py-[7px] text-sm text-[#9FEBD9] transition-colors hover:bg-cat-teal/20"
      :title="t('profiles.displayHint', { name: displayedProfile?.name ?? '' })"
      @click="display()"
    >
      <MonitorUp :size="15" aria-hidden="true" />
      {{ t("profiles.display") }}
    </button>
  </div>
</template>

<style scoped>
@reference "../style.css";
.menu-item {
  @apply flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm hover:bg-surface-active disabled:pointer-events-none disabled:opacity-40;
}
.menu-item > svg {
  @apply shrink-0;
}
</style>
