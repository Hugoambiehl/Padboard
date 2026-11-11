// What an action becomes when its type is chosen in the inspector: a draft
// the desktop accepts (values not filled in yet are allowed); and the
// subtitle the editor writes for an action.
import { hotkeyLabel } from "./keys";
import type {
  Action,
  MediaCommand,
  MicrophoneCommand,
  NavigateTo,
  ObsCommand,
  VolumeCommand,
} from "../generated/types";

export const ACTION_KINDS = [
  "hotkey",
  "media",
  "volume",
  "microphone",
  "open",
  "lock",
  "navigate",
  "multi",
  "obs",
] as const;
export type ActionKind = (typeof ACTION_KINDS)[number];

export function defaultAction(kind: ActionKind): Action {
  switch (kind) {
    case "hotkey":
      return { type: "hotkey", params: { keys: [] } };
    case "media":
      return { type: "media", params: { command: "play_pause" as MediaCommand } };
    case "volume":
      return { type: "volume", params: { command: "up" as VolumeCommand } };
    case "microphone":
      return { type: "microphone", params: { command: "toggle_mute" as MicrophoneCommand } };
    case "open":
      return { type: "open", params: { target: "" } };
    case "lock":
      return { type: "lock" };
    case "navigate":
      return { type: "navigate", params: { to: "next" as NavigateTo } };
    case "multi":
      return { type: "multi", params: { steps: [] } };
    case "obs":
      return { type: "obs", params: { command: "toggle_record" as ObsCommand } };
  }
}

/** Actions whose button can look "active" (state shown on the tablet). */
export function hasActiveState(action: Action): boolean {
  switch (action.type) {
    case "microphone":
    case "obs":
      return true;
    case "volume":
      return action.params.command === "toggle_mute";
    default:
      return false;
  }
}

/** The subtitle the editor writes for an action ("" for none): the keys of a shortcut. */
export function autoSubtitle(action: Action): string {
  return action.type === "hotkey" && action.params.keys.length > 0
    ? hotkeyLabel(action.params.keys)
    : "";
}

/**
 * The subtitle to set when `before` becomes `after`: the new automatic text
 * if the current subtitle is still the automatic text of `before`,
 * `undefined` (keep it) if the user wrote their own or nothing changes.
 */
export function subtitleAfterChange(
  current: string | undefined,
  before: Action,
  after: Action,
): string | undefined {
  const old = autoSubtitle(before);
  const next = autoSubtitle(after);
  return (current ?? "") === old && next !== old ? next : undefined;
}
