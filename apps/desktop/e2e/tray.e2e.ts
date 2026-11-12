// Notification area: closing the window hides it there (explained the first
// time), and the "start with the session" setting (simulated in test mode:
// the real autostart entry is never touched).
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, literal, t } from "./harness";

/** A StatusNotifier host is running (GNOME: the AppIndicator extension). */
function trayHost(): boolean {
  try {
    const out = execFileSync("gdbus", [
      "call",
      "--session",
      "--dest=org.freedesktop.DBus",
      "--object-path=/org/freedesktop/DBus",
      "--method=org.freedesktop.DBus.NameHasOwner",
      "org.kde.StatusNotifierWatcher",
    ]);
    return out.toString().includes("true");
  } catch {
    return false;
  }
}

const AUTOSTART_ENTRY = join(homedir(), ".config", "autostart", "Padboard.desktop");

let app: App;
beforeAll(async () => {
  app = await App.start();
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

const hiddenCount = async () =>
  (await app.log()).split("\n").filter((l) => l.includes("window hidden in the tray")).length;
const requestClose = () =>
  app.session.execute(
    "window.__TAURI_INTERNALS__.invoke('plugin:window|close', { label: 'main' })",
  );
const visible = () =>
  app.session.executeAsync<boolean>(
    "return await window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', { label: 'main' })",
  );

describe("tray and startup", () => {
  it("the startup setting is off by default and simulated in test mode", async () => {
    const entryBefore = existsSync(AUTOSTART_ENTRY);
    await app.session.click(
      await app.session.find(`//button[@title=${literal(t("app.connectionsHint"))}]`),
    );
    const toggle = await app.session.find("[data-autostart-toggle]");
    expect(await app.session.attribute(toggle, "aria-checked")).toBe("false");
    await app.session.click(toggle);
    await app.session.waitUntil(
      async () => (await app.log()).includes("start with the session enabled=true"),
      "the setting should reach the backend",
    );
    expect(await app.session.attribute(toggle, "aria-checked")).toBe("true");
    expect(existsSync(AUTOSTART_ENTRY)).toBe(entryBefore);
    await app.shot("startup-card");
    await app.session.click(toggle);
  });

  it.skipIf(!trayHost())("the first close explains the tray, then hides the window", async () => {
    await requestClose();
    const dialog = await app.session.find("[data-close-to-tray]");
    expect(await app.session.text(dialog)).toContain(t("tray.closeTitle"));
    expect(await visible()).toBe(true);
    await app.shot("close-to-tray");

    await app.session.click(await app.session.find("[data-close-hide]"));
    await app.session.waitUntil(async () => (await hiddenCount()) === 1, "the window should hide");
    expect(await visible()).toBe(false);
    const prefs = JSON.parse(await app.readConfigFile("preferences.json"));
    expect(prefs.closeHintShown).toBe(true);
  });

  it.skipIf(!trayHost())("the next closes hide it at once", async () => {
    await requestClose();
    await app.session.waitUntil(async () => (await hiddenCount()) === 2, "the window should hide");
    expect(await app.session.findAll("[data-close-to-tray]")).toHaveLength(0);
  });

  it("quitting exits the app", async () => {
    await app.close();
  });
});
