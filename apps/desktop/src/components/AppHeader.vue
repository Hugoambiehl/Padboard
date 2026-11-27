<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { Download, MonitorSmartphone, Redo2, Undo2 } from "lucide-vue-next";
import ProfileMenu from "./ProfileMenu.vue";
import { useEditor } from "../composables/useEditor";
import { useStatus } from "../composables/useStatus";
import { useUpdates } from "../composables/useUpdates";

defineProps<{ view: "editor" | "connections" }>();
defineEmits<{ pair: []; toggleView: []; showUpdates: [] }>();
const { t } = useI18n();
const { canUndo, canRedo, undo, redo } = useEditor();
const { snapshot } = useStatus();
const { updates } = useUpdates();
/** The version to offer in the header, once one is found. */
const newVersion = computed(() => {
  const status = updates.value?.status;
  return status?.state === "available" || status?.state === "ready" ? status.version : null;
});

const clients = computed(() => snapshot.value?.clients ?? []);
/** The second screen, always shown while it runs (with "Arrêter"). */
const screen = computed(() => snapshot.value?.secondScreen ?? null);
function stopScreen() {
  void invoke("stop_second_screen");
}
/** "Galaxy Tab A · USB" for one tablet, "2 tablettes" for more. */
const pill = computed(() => {
  const list = clients.value;
  if (list.length === 1) {
    const c = list[0]!;
    return `${c.deviceName} · ${t(`clients.via.${c.transport}`)}`;
  }
  return t("app.tablets", list.length);
});
</script>

<template>
  <header
    class="flex flex-wrap items-center justify-between gap-3 border-b border-line bg-surface px-5 py-3"
  >
    <div class="flex flex-wrap items-center gap-3.5">
      <!-- The app icon (assets/icon/padboard.svg, small-size variant). -->
      <img src="/favicon.svg" alt="" class="size-8" />
      <span class="font-display text-[17px] font-semibold">{{ t("app.name") }}</span>
      <span class="h-[22px] w-px bg-[#2A2D36]" />
      <ProfileMenu />
      <div class="flex gap-1">
        <button
          type="button"
          class="icon-button"
          :aria-label="t('editor.undo')"
          :title="t('editor.undoShortcut')"
          :disabled="!canUndo"
          @click="undo()"
        >
          <Undo2 :size="16" aria-hidden="true" />
        </button>
        <button
          type="button"
          class="icon-button"
          :aria-label="t('editor.redo')"
          :title="t('editor.redoShortcut')"
          :disabled="!canRedo"
          @click="redo()"
        >
          <Redo2 :size="16" aria-hidden="true" />
        </button>
      </div>
    </div>
    <div class="flex flex-wrap items-center gap-2.5">
      <div
        v-if="screen"
        class="flex items-center gap-2 rounded-full border border-cat-blue/40 bg-cat-blue/12 py-1 pr-1 pl-3 text-[13px] text-[#BFDCFF]"
        data-second-screen
      >
        <MonitorSmartphone :size="14" aria-hidden="true" />
        {{ t("app.secondScreen", { name: screen.deviceName }) }}
        <button
          type="button"
          class="rounded-full bg-cat-blue/20 px-2.5 py-0.5 text-xs hover:bg-cat-blue/30"
          @click="stopScreen"
        >
          {{ t("app.stopSecondScreen") }}
        </button>
      </div>
      <button
        v-if="newVersion"
        type="button"
        class="flex items-center gap-2 rounded-full border border-accent/40 bg-accent/12 px-3 py-1.5 text-[13px] text-[#C9C2FF] transition-colors hover:bg-accent/20"
        data-update-badge
        @click="$emit('showUpdates')"
      >
        <Download :size="14" aria-hidden="true" />
        {{ t("updates.badge", { version: newVersion }) }}
      </button>
      <button
        type="button"
        class="flex items-center gap-2 rounded-full border px-3 py-1.5 text-[13px] transition-colors"
        :class="
          clients.length > 0
            ? 'border-cat-teal/35 bg-cat-teal/10 text-[#9FEBD9] hover:bg-cat-teal/20'
            : 'border-line-2 bg-surface-2 text-muted hover:bg-surface-active'
        "
        :aria-pressed="view === 'connections'"
        :title="t('app.connectionsHint')"
        @click="$emit('toggleView')"
      >
        <span
          class="size-2 rounded-full"
          :class="clients.length > 0 ? 'bg-cat-teal' : 'bg-[#3A3D47]'"
        />
        {{ pill }}
      </button>
      <button
        type="button"
        class="rounded-[10px] border border-line-2 bg-surface-2 px-3.5 py-[7px] text-sm transition-colors hover:bg-surface-active"
        @click="$emit('pair')"
      >
        {{ t("pairing.open") }}
      </button>
    </div>
  </header>
</template>

<style scoped>
@reference "../style.css";
.icon-button {
  @apply flex size-[34px] items-center justify-center rounded-[9px] border border-line-2 text-[#C3C8D4] transition-colors hover:bg-surface-active disabled:pointer-events-none disabled:opacity-35;
}
</style>
