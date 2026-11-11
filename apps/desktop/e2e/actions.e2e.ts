// The inspector's action forms: every kind, saved as the desktop expects,
// and the active look shown on the (fake) tablet.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, it } from "vitest";
import { App, FakeTablet, t } from "./harness";
import { Key } from "./webdriver";

let app: App;
let tablet: FakeTablet;
beforeAll(async () => {
  app = await App.start();
  tablet = await app.connectTablet("e2e-actions");
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

interface Saved {
  id: string;
  subtitle?: string;
  action: { type: string; params?: Record<string, unknown> };
  activeLook?: { label?: string; color?: string };
}

/** The saved "copy" button (cell 0,1 of the demo), the one these tests edit. */
async function copy(): Promise<Saved> {
  const text = await readFile(join(app.dir, "config", "profiles.json"), "utf8");
  const buttons: Saved[] = JSON.parse(text).profiles[0].pages[0].buttons;
  return buttons.find((b) => b.id === "copy")!;
}

async function saved(check: (b: Saved) => boolean, what: string) {
  await app.session.waitUntil(async () => check(await copy()), what);
}

const form = "[data-testid=action-form]";
/** A field of the top-level form (not of a step). */
const field = (name: string) =>
  `${form} > label [data-field=${name}], ${form} > label > [data-field=${name}]`;

/**
 * Picks an option of a native select. WebKitWebDriver does not fire `change`
 * when an <option> is clicked, so the value is set and `change` dispatched,
 * as the browser does when the user chooses.
 */
async function choose(selector: string, value: string) {
  const select = await app.session.find(selector);
  const ok = await app.session.execute<boolean>(
    `const [el, v] = arguments;
     if (![...el.options].some((o) => o.value === v)) return false;
     el.value = v;
     el.dispatchEvent(new Event("change", { bubbles: true }));
     return true;`,
    select,
    value,
  );
  if (!ok) throw new Error(`no option ${value} in ${selector}`);
}

describe("action forms", () => {
  it("change the kind and the parameters", async () => {
    await app.select(0, 1);
    await choose(field("kind"), "volume");
    // "Ctrl + C" was the automatic subtitle of the shortcut: it goes.
    await saved((b) => b.action.type === "volume" && b.subtitle === undefined, "a volume action");
    await choose(field("command"), "set");
    await saved(
      (b) => b.action.params?.command === "set" && b.action.params.value === 50,
      "set to 50 %",
    );
    const level = await app.session.find(field("value"));
    await app.session.clear(level);
    await app.typeText(level, "30");
    await app.session.chord(Key.Tab);
    await saved((b) => b.action.params?.value === 30, "set to 30 %");
    await app.shot("40-volume-form");
  });

  it("type an open target, undone in one step", async () => {
    await choose(field("kind"), "open");
    const target = await app.session.find(field("target"));
    await app.typeText(target, "https://example.org");
    await saved((b) => b.action.params?.target === "https://example.org", "the URL");
    await app.select(0, 0); // leave the field
    await app.session.chord(Key.Control, "z");
    await saved(
      (b) => b.action.type === "open" && b.action.params?.target === "",
      "the URL undone at once",
    );
  });

  it("navigate to a page chosen in a list", async () => {
    await app.select(0, 1);
    await choose(field("kind"), "navigate");
    await choose(field("to"), "page");
    await choose(field("id"), "page-1");
    await saved((b) => b.action.params?.to === "page" && b.action.params.id === "page-1", "page 1");
  });

  it("build a multi action step by step", async () => {
    await choose(field("kind"), "multi");
    await app.clickText(t("actions.form.addStep"), "button");
    await app.clickText(t("actions.form.addStep"), "button");
    await choose("[data-step='0'] [data-field=kind]", "microphone");
    await choose("[data-step='1'] [data-field=kind]", "navigate");
    const delay = await app.session.find("[data-step='0'] [data-field=delay]");
    await app.session.clear(delay);
    await app.typeText(delay, "200");
    await app.session.chord(Key.Tab);
    await saved((b) => {
      const steps = (b.action.params?.steps ?? []) as {
        action: { type: string };
        delayMs: number;
      }[];
      return (
        steps.length === 2 && steps[0]!.action.type === "microphone" && steps[0]!.delayMs === 200
      );
    }, "two steps, a pause after the first");
    await app.shot("41-multi-form");
    await app.clickLabel(t("actions.form.moveUp", { n: 2 }));
    await saved((b) => {
      const steps = (b.action.params?.steps ?? []) as { action: { type: string } }[];
      return steps[0]?.action.type === "navigate";
    }, "the navigation first");
    await app.clickLabel(t("actions.form.removeStep", { n: 1 }));
    await saved((b) => ((b.action.params?.steps ?? []) as unknown[]).length === 1, "one step left");
  });

  it("show a custom look on the tablet while active", async () => {
    await choose(field("kind"), "microphone");
    await app.session.find("[data-testid=active-look]");
    const label = await app.session.find("[data-field=active-label]");
    await app.typeText(label, "Chut");
    await app.clickLabel(`${t("inspector.activeLook.color")} ${t("colors.orange")}`);
    await saved(
      (b) => b.activeLook?.label === "Chut" && b.activeLook.color === "orange",
      "the look",
    );
    await app.shot("42-active-look");

    const from = tablet.lines.length;
    await app.setState({ audio: { outputPercent: 50, outputMuted: false, inputMuted: true } });
    await tablet.waitFor(/^Update copy: "Chut" active/, from);
  });

  it("name an OBS scene, shown active when it is the current one", async () => {
    await choose(field("kind"), "obs");
    await choose(field("command"), "set_scene");
    // OBS is not connected in this mode: the name is typed.
    const scene = await app.session.find(field("target"));
    await app.typeText(scene, "Jeu");
    await saved((b) => b.action.params?.target === "Jeu", "the scene");
    const from = tablet.lines.length;
    await app.setState({ obs: { scene: "Jeu" } });
    await tablet.waitFor(/^Update copy: .* active \(Scène active\)/, from);
    await app.setState({ obs: "unavailable" });
    await tablet.waitFor(/^Update copy: .*unavailable/, from);
  });
});
