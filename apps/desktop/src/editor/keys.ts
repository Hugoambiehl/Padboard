// Human-readable key names (W3C `KeyboardEvent.code` → label shown in the UI).

export const isWindows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

const named: Record<string, string> = {
  ControlLeft: "Ctrl",
  ControlRight: "Ctrl droit",
  ShiftLeft: "Maj",
  ShiftRight: "Maj droite",
  AltLeft: "Alt",
  AltRight: "Alt Gr",
  MetaLeft: isWindows ? "Win" : "Super",
  MetaRight: isWindows ? "Win droite" : "Super droite",
  Space: "Espace",
  Enter: "Entrée",
  Escape: "Échap",
  Backspace: "Retour arrière",
  Tab: "Tab",
  CapsLock: "Verr. maj",
  Delete: "Suppr",
  Insert: "Inser",
  Home: "Début",
  End: "Fin",
  PageUp: "Page préc.",
  PageDown: "Page suiv.",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  PrintScreen: "Impr. écran",
  ScrollLock: "Arrêt défil.",
  Pause: "Pause",
  ContextMenu: "Menu",
  NumLock: "Verr. num",
  MediaPlayPause: "Lecture/Pause",
  MediaStop: "Stop",
  MediaTrackNext: "Piste suiv.",
  MediaTrackPrevious: "Piste préc.",
  AudioVolumeUp: "Volume +",
  AudioVolumeDown: "Volume −",
  AudioVolumeMute: "Muet",
};

/**
 * Label of a key. Keys are physical positions named after the US layout
 * (`KeyQ` is the "A" key on AZERTY): letters and digits show their US name.
 */
export function keyLabel(code: string): string {
  if (named[code]) return named[code];
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^Numpad[0-9]$/.test(code)) return `Pavé ${code.slice(6)}`;
  if (/^F[0-9]{1,2}$/.test(code)) return code;
  return code;
}

export function hotkeyLabel(keys: string[]): string {
  return keys.map(keyLabel).join(" + ");
}
