import { computed, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import * as commands from "../commands";
import type {
  Edit,
  EditError,
  EditOutcome,
  EditorState,
  PageConfig,
  ProfileConfig,
} from "../generated/types";

// The configuration lives in Rust: this is a read-only mirror, refreshed by
// every `editor` event, and changed only through `edit()`.
const state = ref<EditorState | null>(null);
const lastError = ref<EditError | null>(null);
let started = false;

async function start() {
  await listen<EditorState>("editor", (event) => {
    state.value = event.payload;
  });
  const initial = await commands.getEditor();
  state.value ??= initial;
}

function isEditError(e: unknown): e is EditError {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Applies an edit; resolves `null` when refused (the error is in `lastError`). */
async function edit(op: Edit): Promise<EditOutcome | null> {
  try {
    const outcome = await commands.applyEdit(op);
    lastError.value = null;
    return outcome;
  } catch (e) {
    lastError.value = isEditError(e) ? e : { code: "unknown", detail: String(e) };
    return null;
  }
}

const config = computed(() => state.value?.config ?? null);
const activeProfile = computed<ProfileConfig | null>(
  () => config.value?.profiles.find((p) => p.id === config.value?.activeProfileId) ?? null,
);

export function useEditor() {
  if (!started) {
    started = true;
    void start().catch((e) => console.error("cannot load the editor", e));
  }
  return {
    state,
    config,
    activeProfile,
    lastError,
    canUndo: computed(() => state.value?.canUndo ?? false),
    canRedo: computed(() => state.value?.canRedo ?? false),
    edit,
    undo: () => commands.undo(),
    redo: () => commands.redo(),
    checkpoint: () => commands.checkpoint(),
    clearError: () => (lastError.value = null),
  };
}

export type { PageConfig, ProfileConfig };
