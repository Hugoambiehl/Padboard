// Folders: created from the library with their button, edited like a page,
// opened on the tablet by a navigate message, deleted.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, FakeTablet, literal, t, withText } from "./harness";

let app: App;
let tablet: FakeTablet;
beforeAll(async () => {
  app = await App.start();
  tablet = await app.connectTablet("e2e-folders");
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

interface SavedButton {
  id: string;
  label: string;
  action: { type: string; params?: { to?: string; id?: string } };
}
interface SavedProfile {
  pages: { buttons: SavedButton[] }[];
  folders: { id: string; name: string; buttons: SavedButton[] }[];
}

async function profile(): Promise<SavedProfile> {
  const text = await readFile(join(app.dir, "config", "profiles.json"), "utf8");
  return JSON.parse(text).profiles[0];
}

const libraryItem = (label: string) =>
  `//aside//button[.//span[normalize-space(.)=${literal(label)}]]`;
const tab = (name: string) => `//*[@role='tab'][normalize-space(.)=${literal(name)}]`;

describe("folders", () => {
  it("are created from the library with the button that opens them", async () => {
    await app.select(2, 3);
    await app.session.click(await app.session.find(libraryItem(t("library.presets.folder.label"))));
    await app.expectCell(2, 3, t("folders.defaultName"));
    await app.session.find(tab(t("folders.defaultName")));
    await app.session.find(withText(t("folders.opens", { name: t("folders.defaultName") })));
    await app.shot("30-folder-created");
  });

  it("are filled like a page", async () => {
    await app.clickText(t("folders.open"), "button");
    const folderTab = await app.session.find(tab(t("folders.defaultName")));
    expect(await app.session.attribute(folderTab, "aria-selected")).toBe("true");
    await app.expectCell(0, 0, app.emptyLabel(0, 0));
    await app.session.click(await app.session.find(libraryItem(t("library.presets.copy.label"))));
    await app.expectCell(0, 0, t("library.presets.copy.label"));
    await app.session.waitUntil(
      async () => (await profile()).folders[0]?.buttons.length === 1,
      "the folder should hold the new button",
    );
    await app.shot("31-folder-filled");
  });

  it("open on the tablet that pressed their button", async () => {
    const saved = await profile();
    const folder = saved.folders[0]!;
    const opener = saved.pages[0]!.buttons.find(
      (b) => b.action.type === "navigate" && b.action.params?.id === folder.id,
    )!;
    const from = tablet.lines.length;
    tablet.press(opener.id);
    await tablet.waitFor(new RegExp(`^Navigate Folder ${folder.id}$`), from);
    // Navigation does not run anything on the PC.
    expect(await app.actions()).toEqual([]);
  });

  it("can be deleted, leaving a flagged button", async () => {
    await app.clickLabel(t("pages.menu"));
    await app.clickText(t("pages.deleteFolder"), "button");
    await app.clickText(t("pages.confirmDelete"), "button");
    await app.session.gone(tab(t("folders.defaultName")));
    await app.select(2, 3);
    await app.session.find(withText(t("folders.missing")));
    await app.shot("32-folder-deleted");
    await app.session.waitUntil(
      async () => (await profile()).folders.length === 0,
      "the folder should be gone",
    );
  });
});
