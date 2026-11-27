// The action library (presets), read once from the backend and shared.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Preset } from "../generated/types";

const presets = ref<Preset[] | null>(null);
let loading: Promise<void> | null = null;

/** The library; `presets` is null until it is loaded. */
export function useLibrary() {
  loading ??= invoke<Preset[]>("library")
    .then((list) => {
      presets.value = list;
    })
    .catch(() => {
      loading = null;
    });
  return { presets, loaded: loading };
}
