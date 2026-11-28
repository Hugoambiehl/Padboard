// The "Second écran" action in the editor: listed only where the PC has a
// picture source (here the test pattern, PADBOARD_SCREEN_SOURCE=pattern),
// explained in the inspector; pressed, the PC's user is asked first (the
// fake tablet's connection is not verified in test mode: asked each time).
// The stream itself is tested in server.rs.
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { Key } from "./webdriver";
import { App, FakeTablet, literal, t, withText } from "./harness";

let app: App;
let tablet: FakeTablet;
/** The buttons the tablet had before the action was added. */
let before: string[];
const ids = (layout: string | undefined) =>
  (layout ?? "").replace(/^Layout "[^"]*": /, "").split(", ");

beforeAll(async () => {
  app = await App.start("fr", { PADBOARD_SCREEN_SOURCE: "pattern" });
  tablet = await app.connectTablet("e2e-screen", true);
  before = ids(tablet.lastLayout());
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

describe("second screen action", () => {
  it("is in the library and explained in the inspector", async () => {
    // Free a cell, then drop the action on it.
    await app.select(2, 4);
    await app.session.chord(Key.Delete);
    await app.expectCell(2, 4, app.emptyLabel(2, 4));
    // The entry is low in the list: search for it, as a user would.
    const search = await app.session.find(
      `input[placeholder=${JSON.stringify(t("library.searchPlaceholder"))}]`,
    );
    await app.typeText(search, t("library.presets.secondScreen.label"));
    const preset = await app.session.find(
      `//aside//button[.//span[normalize-space(.)=${literal(t("library.presets.secondScreen.label"))}]]`,
    );
    await app.session.drag(preset, await app.cell(2, 4));
    await app.expectCell(2, 4, t("library.presets.secondScreen.label"));
    await app.select(2, 4);
    await app.session.find(withText(t("actions.form.secondScreenHint"), "p"));
    await app.shot("50-second-screen");
  });

  it("asks the PC's user before the stream is offered", async () => {
    // The new button's id, from the layout the tablet received.
    const deadline = Date.now() + 5000;
    let added: string | undefined;
    while (!added && Date.now() < deadline) {
      added = ids(tablet.lastLayout()).find((id) => !before.includes(id));
      if (!added) await new Promise((r) => setTimeout(r, 100));
    }
    expect(added).toBeDefined();

    let from = tablet.lines.length;
    tablet.press(added!);
    await app.session.find("[data-screen-request]");
    await app.session.find(withText(t("screenRequest.notRemembered"), "p"));
    await app.shot("51-second-screen-question");
    await app.session.click(await app.session.find("[data-screen-refuse]"));
    await tablet.waitFor(/^Error from the PC: ActionFailed .*refused/, from);

    from = tablet.lines.length;
    tablet.press(added!);
    await app.session.click(await app.session.find("[data-screen-allow]"));
    await tablet.waitFor(/^Screen offer H264 1728x1080/, from);
  });
});
