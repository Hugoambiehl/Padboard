// Records a shortcut from keyboard events (`KeyboardEvent.code`, the same
// physical key names as the desktop). Pure, unit-tested.

export const MAX_KEYS = 6;

const MODIFIER_ORDER = [
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
];

/** Modifiers first (Ctrl, Shift, Alt, Meta), then the other keys in press order. */
export function normalize(keys: string[]): string[] {
  const unique = [...new Set(keys)];
  const modifiers = MODIFIER_ORDER.filter((m) => unique.includes(m));
  const others = unique.filter((k) => !MODIFIER_ORDER.includes(k));
  return [...modifiers, ...others].slice(0, MAX_KEYS);
}

export class HotkeyRecorder {
  private readonly held = new Set<string>();
  private readonly pressed: string[] = [];

  constructor(private readonly validKeys: ReadonlySet<string>) {}

  /** A key went down. Unknown keys (not injectable) are ignored. */
  down(code: string): void {
    if (!this.validKeys.has(code) || this.held.has(code)) return;
    this.held.add(code);
    if (!this.pressed.includes(code)) this.pressed.push(code);
  }

  /** A key went up; returns the shortcut once every key is released. */
  up(code: string): string[] | null {
    this.held.delete(code);
    if (this.held.size > 0 || this.pressed.length === 0) return null;
    return normalize(this.pressed);
  }

  /** Keys seen so far (for live feedback). */
  get current(): string[] {
    return normalize(this.pressed);
  }
}
