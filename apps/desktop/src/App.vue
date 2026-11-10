<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEventListener } from "@vueuse/core";
import AppHeader from "./components/AppHeader.vue";
import PairingDialog from "./components/PairingDialog.vue";
import EditorView from "./views/EditorView.vue";
import ConnectionsView from "./views/ConnectionsView.vue";
import { useStatus } from "./composables/useStatus";
import { useEditor } from "./composables/useEditor";

const { t } = useI18n();
const { snapshot, failed } = useStatus();
const { undo, redo } = useEditor();
// `?view=connections` opens the connections view (browser preview, see src/dev/mock.ts).
const view = ref<"editor" | "connections">(
  import.meta.env.DEV && new URLSearchParams(location.search).get("view") === "connections"
    ? "connections"
    : "editor",
);
const pairingOpen = ref(false);

// Undo / redo everywhere, text fields included: the configuration's history
// (typing in a field is one entry) replaces the fields' native undo.
useEventListener(window, "keydown", (event: KeyboardEvent) => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
  const key = event.key.toLowerCase();
  if (key === "z" && !event.shiftKey) {
    event.preventDefault();
    void undo();
  } else if ((key === "z" && event.shiftKey) || key === "y") {
    event.preventDefault();
    void redo();
  }
});
</script>

<template>
  <div class="flex h-screen flex-col">
    <AppHeader
      :view="view"
      @pair="pairingOpen = true"
      @toggle-view="view = view === 'editor' ? 'connections' : 'editor'"
    />
    <p v-if="failed" class="p-5 text-sm text-cat-red">{{ t("app.loadError") }}</p>
    <p v-else-if="!snapshot" class="p-5 text-sm text-muted">{{ t("app.loading") }}</p>
    <template v-else>
      <EditorView v-show="view === 'editor'" />
      <div v-if="view === 'connections'" class="min-h-0 flex-1 overflow-y-auto">
        <ConnectionsView :snapshot="snapshot" @back="view = 'editor'" @pair="pairingOpen = true" />
      </div>
      <PairingDialog
        v-if="pairingOpen"
        :pairing="snapshot.pairing"
        :wifi="snapshot.server.wifi"
        @close="pairingOpen = false"
      />
    </template>
  </div>
</template>
