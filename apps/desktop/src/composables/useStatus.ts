import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Snapshot } from "../types/status";

// One subscription for the whole app (header, editor and connections view).
const snapshot = ref<Snapshot | null>(null);
const failed = ref(false);
let started = false;

async function start() {
  try {
    // Subscribe first so no update is lost between the fetch and the subscription.
    await listen<Snapshot>("status", (event) => {
      snapshot.value = event.payload;
    });
    const initial = await invoke<Snapshot>("get_status");
    snapshot.value ??= initial;
  } catch (error) {
    console.error("cannot reach the Padboard backend", error);
    failed.value = true;
  }
}

/** Live server status: initial fetch, then every `status` event from the backend. */
export function useStatus() {
  if (!started) {
    started = true;
    void start();
  }
  return { snapshot, failed };
}
