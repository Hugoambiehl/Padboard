// Human-readable key names (W3C `KeyboardEvent.code` → label shown in the
// UI), in the UI language (`keys` in src/locales).
import { i18n } from "../i18n";

export const isWindows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

/**
 * Label of a key. Keys are physical positions named after the US layout
 * (`KeyQ` is the "A" key on AZERTY): letters and digits show their US name.
 */
export function keyLabel(code: string): string {
  const { t, te } = i18n.global;
  const named = isWindows && code.startsWith("Meta") ? `keys.${code}Win` : `keys.${code}`;
  if (te(named)) return t(named);
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^Numpad[0-9]$/.test(code)) return t("keys.numpad", { n: code.slice(6) });
  return code;
}

export function hotkeyLabel(keys: string[]): string {
  return keys.map(keyLabel).join(" + ");
}
