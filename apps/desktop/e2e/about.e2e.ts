// "À propos" card and the "Licences" dialog (needs scripts/licenses.sh,
// run by scripts/desktop-e2e.sh before the build).
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, literal, t } from "./harness";

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

describe("about and licenses", () => {
  it("the connections view shows the version and the license", async () => {
    await app.session.click(
      await app.session.find(`//button[@title=${literal(t("app.connectionsHint"))}]`),
    );
    const card = await app.session.find(
      `//section[.//h2[normalize-space(.)=${literal(t("about.title"))}]]`,
    );
    const text = await app.session.text(card);
    expect(text).toContain(t("about.version", { version: "0.1.0" }));
    expect(text).toContain(t("about.license"));
  });

  it("the licenses dialog lists Padboard and the third-party components", async () => {
    await app.session.click(await app.session.find("[data-licenses-open]"));
    await app.session.find("[data-licenses-list] li");
    // Padboard's own license, then a crate and the bundled fonts.
    await app.session.click(
      await app.session.find(
        `//dialog//button[span[normalize-space(.)=${literal(t("app.name"))}]]`,
      ),
    );
    await app.session.find(`//pre[contains(., "GNU GENERAL PUBLIC LICENSE")]`);
    await app.shot("licenses");

    const search = await app.session.find("[data-licenses-search]");
    await app.typeText(search, "tokio");
    await app.session.waitUntil(async () => {
      const rows = await app.session.findAll("[data-licenses-list] li");
      return rows.length > 0 && rows.length < 10;
    }, "the search should narrow the list");
    await app.session.click(await app.session.find("[data-licenses-list] li button"));
    await app.session.find(`//ul[@data-licenses-list]//pre[contains(., "MIT")]`);

    await app.fill(search, "@fontsource/inter");
    await app.session.click(await app.session.find("[data-licenses-list] li button"));
    await app.session.find(`//ul[@data-licenses-list]//pre[contains(., "SIL OPEN FONT LICENSE")]`);
    await app.shot("licenses-font");
  });
});
