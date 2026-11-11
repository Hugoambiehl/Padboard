import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getObs, type ObsInfo, type ObsStatus } from "../commands";

// OBS settings and connection status, shared (connections view, editor pickers).
const info = ref<ObsInfo | null>(null);
let started = false;

async function refresh() {
  info.value = await getObs();
}

export function useObs() {
  if (!started) {
    started = true;
    void refresh().catch((e) => console.error("cannot read the OBS settings", e));
    // Status changes; the scene and input lists come with a refresh.
    void listen<ObsStatus>("obs", (event) => {
      if (info.value?.available) info.value = { ...info.value, status: event.payload };
      void refresh();
    });
  }
  return { info, refresh };
}
