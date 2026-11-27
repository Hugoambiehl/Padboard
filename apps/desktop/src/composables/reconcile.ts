/**
 * `next`, sharing with `prev` every part that did not change (the same
 * object, `prev`'s own when nothing changed). A component whose props keep
 * their identity is not rendered again: a new snapshot of the backend (a
 * tablet reconnecting) only redraws what it changed.
 *
 * For JSON data (plain objects, arrays, primitives).
 */
export function reconcile<T>(prev: T, next: T): T {
  if (Object.is(prev, next)) return prev;
  if (Array.isArray(prev) && Array.isArray(next)) {
    const items = next.map((item, i) => (i < prev.length ? reconcile(prev[i], item) : item));
    const same = items.length === prev.length && items.every((item, i) => item === prev[i]);
    return (same ? prev : items) as T;
  }
  if (isPlainObject(prev) && isPlainObject(next)) {
    const out: Record<string, unknown> = {};
    let same = Object.keys(prev).length === Object.keys(next).length;
    for (const key of Object.keys(next)) {
      out[key] = key in prev ? reconcile(prev[key], next[key]) : next[key];
      if (!(key in prev) || out[key] !== prev[key]) same = false;
    }
    return (same ? prev : out) as T;
  }
  return next;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
