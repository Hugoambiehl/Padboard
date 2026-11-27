// The "Second écran" action in the editor: listed only where the PC has a
// picture source (here the test pattern, PADBOARD_SCREEN_SOURCE=pattern),
// explained in the inspector. The stream itself is tested in server.rs.
import { afterAll, afterEach, beforeAll, describe, it } from "vitest";
import { Key } from "./webdriver";
import { App, literal, t, withText } from "./harness";

let app: App;
beforeAll(async () => {
  app = await App.start("fr", { PADBOARD_SCREEN_SOURCE: "pattern" });
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
});
