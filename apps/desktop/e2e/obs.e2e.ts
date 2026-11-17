// OBS scene lists in the editor (issue #13): OBS (the fake-obs example)
// starts after the editor loaded, then scenes change while it is connected.
// The app reaches it for real (PADBOARD_TEST_OBS=1), not through state.json.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, it } from "vitest";
import { App, type FakeObs, freePort } from "./harness";

let app: App;
let obs: FakeObs;
let obsPort: number;
beforeAll(async () => {
  obsPort = await freePort();
  // Nothing listens on that port yet: OBS is "closed" when the editor loads.
  app = await App.start(
    "fr",
    { PADBOARD_TEST_OBS: "1" },
    { "obs.json": { version: 1, enabled: true, host: "127.0.0.1", port: obsPort } },
  );
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

const form = "[data-testid=action-form]";
const field = (name: string) =>
  `${form} > label [data-field=${name}], ${form} > label > [data-field=${name}]`;

/** Same as in actions.e2e.ts: WebKitWebDriver does not fire `change` on a click. */
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

/** The scenes offered by the target list, or null while it is a text field. */
function scenes(): Promise<string[] | null> {
  return app.session.execute<string[] | null>(
    `const el = document.querySelector(arguments[0]);
     return el && el.tagName === "SELECT"
       ? [...el.options].map((o) => o.value).filter(Boolean)
       : null;`,
    field("target"),
  );
}

async function expectScenes(expected: string[], timeout = 5000) {
  let last: string[] | null = null;
  try {
    await app.session.waitUntil(
      async () => {
        last = await scenes();
        return JSON.stringify(last) === JSON.stringify(expected);
      },
      `the scene list should be ${JSON.stringify(expected)}`,
      timeout,
    );
  } catch (e) {
    throw new Error(`${(e as Error).message}; it is ${JSON.stringify(last)}`, { cause: e });
  }
}

describe("OBS scene list", () => {
  it("fills when OBS starts after the editor loaded", async () => {
    await app.select(0, 1);
    await choose(field("kind"), "obs");
    await choose(field("command"), "set_scene");
    // OBS not running: the name would be typed.
    if ((await scenes()) !== null) throw new Error("the target should be a text field");
    obs = await app.startObs(obsPort, ["Jeu", "Pause"]);
    // The app retries every 3 s; no reload of the editor.
    await obs.waitFor(/^identified/, 0, 10_000);
    await expectScenes(["Jeu", "Pause"]);
    await app.shot("90-obs-scenes");
  });

  it("follows a scene created then renamed in OBS", async () => {
    await obs.command("create Discussion");
    await expectScenes(["Jeu", "Pause", "Discussion"]);
    await obs.command("rename Pause Repos");
    await expectScenes(["Jeu", "Repos", "Discussion"]);
    await obs.command("remove Jeu");
    await expectScenes(["Repos", "Discussion"]);
  });

  it("saves a scene picked in the list", async () => {
    await choose(field("target"), "Repos");
    await app.session.waitUntil(async () => {
      const text = await readFile(join(app.dir, "config", "profiles.json"), "utf8");
      const copy = JSON.parse(text).profiles[0].pages[0].buttons.find(
        (b: { id: string }) => b.id === "copy",
      );
      return copy?.action?.params?.target === "Repos";
    }, "the scene saved");
  });
});
