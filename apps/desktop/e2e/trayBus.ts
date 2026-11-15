// The app's tray icon seen from the session bus, as the desktop sees it:
// StatusNotifierItem (registered with org.kde.StatusNotifierWatcher) and its
// menu (com.canonical.dbusmenu). The tray tests use it while the window is
// hidden: no WebDriver command may reach a hidden window (WebKitWebDriver
// hung on it, then crashed with a segfault when the session ended).
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";

const run = promisify(execFile);

async function gdbus(dest: string, path: string, method: string, ...args: string[]) {
  const { stdout } = await run("gdbus", [
    "call",
    "--session",
    "--timeout",
    "5",
    "--dest",
    dest,
    "--object-path",
    path,
    "--method",
    method,
    "--",
    ...args,
  ]);
  return stdout;
}

export interface TrayItem {
  /** Unique bus name of the app's connection (`:1.N`). */
  bus: string;
  /** Process of the app. */
  pid: number;
  /** Object path of its menu. */
  menu: string;
}

/** The PADBOARD_TEST_DIR of a process (`null`: none, or not readable). */
async function testDirOf(pid: number): Promise<string | null> {
  const env = await readFile(`/proc/${pid}/environ`, "utf8").catch(() => "");
  const entry = env.split("\0").find((e) => e.startsWith("PADBOARD_TEST_DIR="));
  return entry ? entry.slice("PADBOARD_TEST_DIR=".length) : null;
}

/**
 * The tray icon of the test app started with PADBOARD_TEST_DIR=`dir`;
 * another Padboard's icon (the user's own) is never picked.
 */
export async function findTrayItem(dir: string): Promise<TrayItem | null> {
  const items = await gdbus(
    "org.kde.StatusNotifierWatcher",
    "/StatusNotifierWatcher",
    "org.freedesktop.DBus.Properties.Get",
    "org.kde.StatusNotifierWatcher",
    "RegisteredStatusNotifierItems",
  );
  // Entries read 'bus@/object/path'.
  for (const match of items.matchAll(/'([^'@]+)@([^']+)'/g)) {
    const [, bus = "", path = ""] = match;
    const owner = await gdbus(
      "org.freedesktop.DBus",
      "/org/freedesktop/DBus",
      "org.freedesktop.DBus.GetConnectionUnixProcessID",
      bus,
    ).catch(() => "");
    const pid = Number(/uint32 (\d+)/.exec(owner)?.[1]);
    if (!pid || (await testDirOf(pid)) !== dir) continue;
    const menu = await gdbus(
      bus,
      path,
      "org.freedesktop.DBus.Properties.Get",
      "org.kde.StatusNotifierItem",
      "Menu",
    );
    const menuPath = /objectpath '([^']+)'/.exec(menu)?.[1];
    if (menuPath) return { bus, pid, menu: menuPath };
  }
  return null;
}

/** The menu's entries with a label: id and text. */
export async function menuItems(item: TrayItem): Promise<{ id: number; label: string }[]> {
  const layout = await gdbus(
    item.bus,
    item.menu,
    "com.canonical.dbusmenu.GetLayout",
    "0",
    "-1",
    '["label"]',
  );
  return [...layout.matchAll(/\((\d+), \{'label': <(['"])(.*?)\2>/g)]
    .map(([, id, , label = ""]) => ({ id: Number(id), label }))
    .filter((entry) => entry.id !== 0);
}

/** Clicks the menu entry labelled `label`, as the desktop does. */
export async function clickMenu(item: TrayItem, label: string): Promise<void> {
  const entry = (await menuItems(item)).find((e) => e.label === label);
  if (!entry) throw new Error(`no "${label}" in the tray menu`);
  await gdbus(
    item.bus,
    item.menu,
    "com.canonical.dbusmenu.Event",
    String(entry.id),
    "clicked",
    "<int32 0>",
    "0",
  );
}

/** The process is still running. */
export function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}
