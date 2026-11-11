// Human-readable summary of an action, for the inspector.
import type { Action } from "../generated/types";

type Translate = (key: string, params?: Record<string, unknown> | number) => string;

/** Short description of what the action does ("Monte le son de 5 %"). */
export function actionSummary(action: Action, t: Translate): string {
  switch (action.type) {
    case "hotkey":
      return t("actions.kinds.hotkey");
    case "media":
      return t(`actions.media.${action.params.command}`);
    case "volume":
      return t(`actions.volume.${action.params.command}`, {
        step: action.params.value ?? 5,
        level: action.params.value ?? 0,
      });
    case "microphone":
      return t(`actions.microphone.${action.params.command}`);
    case "open":
      return action.params.target
        ? t("actions.open", { target: action.params.target })
        : t("actions.openEmpty");
    case "lock":
      return t("actions.lock");
    case "navigate":
      return t(`actions.navigate.${action.params.to}`);
    case "multi":
      return t("actions.multi", action.params.steps.length);
    case "obs":
      return t(`actions.obs.${action.params.command}`, { target: action.params.target ?? "" });
  }
}

/** Run by the PC itself (the "Tester" button can try them). */
export function isSystemAction(action: Action): boolean {
  return ["hotkey", "media", "volume", "microphone", "open", "lock"].includes(action.type);
}
