<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check } from "lucide-vue-next";
import PageTabs from "../components/editor/PageTabs.vue";
import GridSizeMenu from "../components/editor/GridSizeMenu.vue";
import ReplicaGrid from "../components/editor/ReplicaGrid.vue";
import ActionLibrary from "../components/editor/ActionLibrary.vue";
import DragGhost from "../components/editor/DragGhost.vue";
import { useEditor } from "../composables/useEditor";
import { useStatus } from "../composables/useStatus";

const { t } = useI18n();
const { activeProfile } = useEditor();
const { snapshot } = useStatus();
const live = computed(() => (snapshot.value?.clients.length ?? 0) > 0);
/** Paths of the library icons, for the drag ghost. */
const presetIcons = ref<Record<string, string[]>>({});
</script>

<template>
  <div class="flex min-h-0 flex-1">
    <aside class="w-[280px] shrink-0 overflow-y-auto border-r border-line bg-surface px-4 py-5">
      <ActionLibrary @icons="presetIcons = $event" />
    </aside>
    <main class="flex min-w-0 flex-1 flex-col items-center gap-5 overflow-y-auto px-8 py-7">
      <p v-if="!activeProfile" class="text-sm text-muted">{{ t("app.loading") }}</p>
      <template v-else>
        <div class="flex w-full max-w-[820px] flex-wrap items-center justify-between gap-3">
          <PageTabs />
          <GridSizeMenu />
        </div>
        <ReplicaGrid />
        <div
          class="flex w-full max-w-[820px] flex-wrap items-center justify-between gap-3 text-[13px] text-muted"
        >
          <span>{{ t("editor.hint") }}</span>
          <span v-if="live" class="flex items-center gap-1.5 text-[#9FEBD9]">
            <Check :size="14" class="text-cat-teal" aria-hidden="true" />
            {{ t("editor.live") }}
          </span>
          <span v-else>{{ t("editor.noTablet") }}</span>
        </div>
      </template>
    </main>
    <aside
      class="w-[340px] shrink-0 overflow-y-auto border-l border-line bg-surface px-[22px] py-6"
    />
    <DragGhost :preset-icons="presetIcons" />
  </div>
</template>
