import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Snapshot } from "../types/status";

/** Live server status: initial fetch, then every `status` event from the backend. */
export function useStatus() {
  const snapshot = ref<Snapshot | null>(null);
  const failed = ref(false);
  let unlisten: UnlistenFn | undefined;

  onMounted(async () => {
    try {
      // Subscribe first so no update is lost between the fetch and the subscription.
      unlisten = await listen<Snapshot>("status", (event) => {
        snapshot.value = event.payload;
      });
      const initial = await invoke<Snapshot>("get_status");
      snapshot.value ??= initial;
    } catch (error) {
      console.error("cannot reach the Padboard backend", error);
      failed.value = true;
    }
  });

  onUnmounted(() => unlisten?.());

  return { snapshot, failed };
}
