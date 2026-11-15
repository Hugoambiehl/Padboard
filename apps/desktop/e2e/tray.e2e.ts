// Notification area: closing the window hides it there (explained the first
// time), and the "start with the session" setting (simulated in test mode:
// the real autostart entry is never touched).
//
// While the window is hidden, no WebDriver command is sent: WebKitWebDriver
// hung on a hidden window, then crashed (segfault) when the session ended,
// which Ubuntu reported as a crash. The hidden state is checked from the
// outside instead (the app's log, its process, its tray icon and menu on the
// session bus), and the window is shown again through the tray menu
// ("Ouvrir Padboard", as a user would) before WebDriver is used again.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, literal, t } from "./harness";
import { alive, clickMenu, findTrayItem, menuItems, type TrayItem } from "./trayBus";

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

/**
 * Why the tray tests cannot run here (`null`: they run). Headless, the window
 * is on a virtual display without a tray, while the session bus still reports
 * the real desktop's host: closing the window then hung, and WebKitWebDriver
 * crashed (SIGSEGV). They stay mandatory with a real window before each
 * release (docs/releasing.md).
 */
const traySkip = process.env.PADBOARD_E2E_HEADLESS
  ? "headless (no tray on the virtual display): run with a real window before a release"
  : trayHost()
    ? null
    : "no StatusNotifier host on this desktop (GNOME: AppIndicator extension)";
if (traySkip) console.warn(`SKIPPED tray tests: ${traySkip}`);
/**
 * Why the window close tests do not run (`null`: they run). Hiding now works
 * (the click is made by the page, after WebDriver's answer), but once the
 * window is shown again from the tray menu, WebKitWebDriver sees no element
 * displayed in it any more. Until that is solved (idea: let the e2e build
 * close the window on a signal, see docs/releasing.md), they run only with
 * PADBOARD_E2E_TRAY_CLOSE=1 and the release checklist has a manual check.
 */
const closeSkip =
  traySkip ??
  (process.env.PADBOARD_E2E_TRAY_CLOSE
    ? null
    : "WebKitWebDriver loses the window shown again from the tray: manual check (docs/releasing.md)");
if (!traySkip && closeSkip) console.warn(`SKIPPED window close tests: ${closeSkip}`);
const closeTest = (name: string) => (closeSkip ? `${name} [skipped: ${closeSkip}]` : name);
/** The test's name, with the reason when it is skipped. */
const trayTest = (name: string) => (traySkip ? `${name} [skipped: ${traySkip}]` : name);

const AUTOSTART_ENTRY = join(homedir(), ".config", "autostart", "Padboard.desktop");

/** Tray texts, from the app (locale.rs, French). */
const OPEN = "Ouvrir Padboard";
const QUIT = "Quitter Padboard";

let app: App;
let tray: TrayItem | null = null;
/** The window is hidden: WebDriver must not be used (see above). */
let hidden = false;

beforeAll(async () => {
  app = await App.start();
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail" && !hidden) {
    await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
  }
});
afterAll(async () => {
  if (hidden && tray) {
    // Never end the session on a hidden window: show it, or end the app first.
    await clickMenu(tray, OPEN).catch(() => undefined);
    await new Promise((r) => setTimeout(r, 1000));
    if (alive(tray.pid)) process.kill(tray.pid, "SIGTERM");
  }
  await app?.stop();
});

const hiddenCount = async () =>
  (await app.log()).split("\n").filter((l) => l.includes("window hidden in the tray")).length;
/**
 * Runs `script` in the page a moment after WebDriver got its answer: an
 * action that hides the window must never run while a WebDriver command is
 * pending (WebKitWebDriver never finishes a command whose effect hides the
 * window).
 */
const later = (script: string) => app.session.execute(`setTimeout(() => { ${script} }, 200)`);
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

  it.skipIf(traySkip)(trayTest("the app has its tray icon and menu"), async () => {
    await app.session.waitUntil(
      async () => (tray = await findTrayItem(app.dir)) !== null,
      "the tray icon should be registered",
      10_000,
    );
    const labels = (await menuItems(tray!)).map((e) => e.label);
    expect(labels).toEqual([OPEN, "Aucune tablette connectée", QUIT]);
  });

  it.skipIf(closeSkip)(
    closeTest("the first close explains the tray, then hides the window"),
    async () => {
      await requestClose();
      const dialog = await app.session.find("[data-close-to-tray]");
      expect(await app.session.text(dialog)).toContain(t("tray.closeTitle"));
      expect(await visible()).toBe(true);
      await app.shot("close-to-tray");

      // From this click on, the window is hidden: no WebDriver command.
      hidden = true;
      await app.session.find("[data-close-hide]");
      await later("document.querySelector('[data-close-hide]').click()");
      await app.session.waitUntil(
        async () => (await hiddenCount()) === 1,
        "the window should hide",
      );
      expect(alive(tray!.pid)).toBe(true);
      expect((await menuItems(tray!)).map((e) => e.label)).toContain(OPEN);
      const prefs = JSON.parse(await app.readConfigFile("preferences.json"));
      expect(prefs.closeHintShown).toBe(true);
    },
  );

  it.skipIf(closeSkip)(closeTest("the tray menu shows the window again"), async () => {
    await clickMenu(tray!, OPEN);
    // Give GTK the time to map the window before WebDriver talks to it.
    await new Promise((r) => setTimeout(r, 1000));
    hidden = false;
    expect(await visible()).toBe(true);
    await app.session.find("[role=grid] [data-cell='0,0']");
  });

  it.skipIf(closeSkip)(closeTest("the next closes hide it at once"), async () => {
    hidden = true;
    await later("window.__TAURI_INTERNALS__.invoke('plugin:window|close', { label: 'main' })");
    await app.session.waitUntil(async () => (await hiddenCount()) === 2, "the window should hide");
    expect(alive(tray!.pid)).toBe(true);
    await clickMenu(tray!, OPEN);
    await new Promise((r) => setTimeout(r, 1000));
    hidden = false;
    // No explanation the second time.
    expect(await app.session.findAll("[data-close-to-tray]")).toHaveLength(0);
  });

  it("quitting exits the app", async () => {
    await app.close();
  });
});
