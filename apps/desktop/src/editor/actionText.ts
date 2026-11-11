// Facts about actions the inspector needs.
import type { Action } from "../generated/types";

/** Run by the PC itself (the "Tester" button can try them). */
export function isSystemAction(action: Action): boolean {
  return ["hotkey", "media", "volume", "microphone", "open", "lock"].includes(action.type);
}
