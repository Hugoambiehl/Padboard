import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getUpdates } from "../commands";
import type { UpdatesView } from "../types/updates";

// The updates' state, shared (header badge, "À propos" card).
const updates = ref<UpdatesView | null>(null);
let started = false;

export function useUpdates() {
  if (!started) {
    started = true;
    void getUpdates()
      .then((view) => {
        updates.value ??= view;
      })
      .catch((e) => console.error("cannot read the updates", e));
    void listen<UpdatesView>("updates", (event) => {
      updates.value = event.payload;
    });
  }
  return { updates };
}
