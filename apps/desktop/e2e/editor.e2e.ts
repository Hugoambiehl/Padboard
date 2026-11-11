// The editor's manual test scenario, on the real window. Steps run in order
// and build on each other (one app, one fresh test configuration).
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, literal, t, withText } from "./harness";
import { Key } from "./webdriver";

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

/** The inspector's text field labelled `label`. */
const field = (label: string) =>
  app.session.find(`//label[span[normalize-space(.)=${literal(label)}]]//input`);
const gridSize = (cols: number, rows: number) => withText(t("grid.size", { cols, rows }), "button");
const tab = (name: string) => `//*[@role='tab'][normalize-space(.)=${literal(name)}]`;
const profileMenu = () => app.session.find(`//button[@aria-haspopup='menu']`);

describe("editor", () => {
  it("starts on the demo profile, in a temporary configuration", async () => {
    await app.expectCell(0, 0, "Terminal");
    await app.expectCell(2, 3, app.emptyLabel(2, 3));
    await app.session.find(withText(t("inspector.noneTitle")));
    await app.shot("01-start");
    const saved = await app.savedConfig().catch(() => null);
    // Seeded on first launch, in the test directory.
    expect(saved?.profiles.map((p) => p.name)).toEqual(["Démo"]);
  });

  it("edits a label, undone in one step", async () => {
    await app.select(0, 0);
    const input = await field(t("inspector.label"));
    await app.session.clear(input);
    await app.typeText(input, "Mon terminal");
    await app.expectCell(0, 0, "Mon terminal");
    await app.shot("02-label");

    // Leave the field (ends the undo entry), then undo with the keyboard.
    await app.select(0, 0);
    await app.session.chord(Key.Control, "z");
    await app.expectCell(0, 0, "Terminal");
    await app.session.chord(Key.Control, Key.Shift, "z");
    await app.expectCell(0, 0, "Mon terminal");
    // The header buttons do the same.
    await app.clickLabel(t("editor.undo"));
    await app.expectCell(0, 0, "Terminal");
    await app.clickLabel(t("editor.redo"));
    await app.expectCell(0, 0, "Mon terminal");
  });

  it("changes the color and the icon", async () => {
    await app.select(0, 0);
    const pink = await app.session.find(`[aria-label=${JSON.stringify(t("colors.pink"))}]`);
    await app.session.click(pink);
    await app.session.waitUntil(
      async () => (await app.session.attribute(pink, "aria-pressed")) === "true",
      "pink should be the button's color",
    );

    // A quick icon, then one from the whole set.
    await app.clickLabel("Moon");
    await app.clickLabel(t("inspector.moreIcons"));
    const search = await app.session.find("dialog[open] input");
    await app.typeText(search, "rocket");
    await app.session.find(`dialog[open] [aria-label='Rocket']`);
    await app.shot("03-icon-picker");
    await app.session.click(await app.session.find(`dialog[open] [aria-label='Rocket']`));
    await app.session.gone("dialog[open]");
    // The selected icon is outlined in the quick choices only when it is one
    // of them: check the saved configuration instead.
    await app.session.waitUntil(async () => {
      const saved = await app.savedConfig();
      const button = saved.profiles[0]!.pages[0]!.buttons.find((b) => b.label === "Mon terminal");
      return (
        JSON.stringify(button).includes('"Rocket"') && JSON.stringify(button).includes('"pink"')
      );
    }, "icon and color should be saved");
    await app.shot("04-appearance");
  });

  it("records a shortcut and tests it without injecting it", async () => {
    await app.select(2, 3);
    await app.clickText(t("inspector.addShortcut"), "button");
    await app.session.find(
      `//label[span[normalize-space(.)=${literal(t("inspector.label"))}]]//input`,
    );
    await app.clickText(t("hotkey.record"), "button");
    await app.session.find(withText(t("hotkey.listening")));
    await app.session.chord(Key.Control, Key.Shift, "k");
    await app.session.gone(withText(t("hotkey.listening")));
    for (const label of ["Ctrl", "Maj", "K"]) {
      await app.session.find(
        `[aria-label=${JSON.stringify(t("hotkey.removeKey", { key: label }))}]`,
      );
    }
    await app.shot("05-hotkey");

    await app.clickText(t("inspector.test"), "button");
    await app.session.find(withText(t("inspector.tested"), "button"));
    expect(await app.actions()).toEqual([
      { type: "hotkey", params: { keys: ["ControlLeft", "ShiftLeft", "KeyK"] } },
    ]);
  });

  it("adds a preset by drag and drop, and moves a button by dragging", async () => {
    // Free a cell: delete the "Verrouiller" button (2,4) with the keyboard.
    await app.select(2, 4);
    await app.session.chord(Key.Delete);
    await app.expectCell(2, 4, app.emptyLabel(2, 4));

    const preset = await app.session.find(
      `//aside//button[.//span[normalize-space(.)=${literal(t("library.presets.cut.label"))}]]`,
    );
    await app.session.drag(preset, await app.cell(2, 4));
    await app.expectCell(2, 4, t("library.presets.cut.label"));

    // Dragging a button onto another swaps them.
    await app.session.drag(await app.cell(0, 1), await app.cell(0, 2));
    await app.expectCell(0, 2, "Copier");
    await app.expectCell(0, 1, "Coller");
    await app.shot("06-drag");
  });

  it("moves the selected button with Alt + arrows", async () => {
    await app.select(0, 1); // Coller
    await app.session.chord(Key.Alt, Key.ArrowLeft);
    await app.expectCell(0, 0, "Coller");
    await app.expectCell(0, 1, "Mon terminal");
    await app.session.chord(Key.Alt, Key.ArrowDown);
    await app.expectCell(1, 0, "Coller");
    await app.expectCell(0, 0, "Lecture / Pause");
    // Arrows alone move the selection.
    await app.session.chord(Key.ArrowRight);
    await app.session.find(withText(t("inspector.position", { row: 2, col: 2 })));
    await app.shot("07-keyboard");
  });

  it("adds, renames, moves and deletes pages", async () => {
    await app.clickLabel(t("pages.add"));
    await app.answerPrompt("Jeux", t("common.create"));
    const jeux = await app.session.find(tab("Jeux"));
    expect(await app.session.attribute(jeux, "aria-selected")).toBe("true");
    await app.expectCell(0, 0, app.emptyLabel(0, 0));

    // Rename and move it before "Page 1".
    await app.clickLabel(t("pages.menu"));
    await app.clickText(t("pages.rename"), "button");
    await app.answerPrompt("Jeux vidéo", t("common.rename"));
    await app.clickLabel(t("pages.menu"));
    await app.clickText(t("pages.moveLeft"), "button");
    // The menu stays open to move again; Escape closes it.
    await app.session.chord(Key.Escape);
    await app.session.gone(withText(t("pages.moveRight"), "button"));
    await app.session.waitUntil(async () => {
      const tabs = await app.session.findAll("[role=tab]");
      const names = await Promise.all(tabs.map((el) => app.session.text(el)));
      return names.join("|") === "Jeux vidéo|Page 1";
    }, "the new page should come first");
    await app.shot("08-pages");

    // Delete it (two clicks: the first asks for confirmation).
    await app.clickLabel(t("pages.menu"));
    await app.clickText(t("pages.delete"), "button");
    await app.clickText(t("pages.confirmDelete"), "button");
    await app.session.gone(tab("Jeux vidéo"));
    await app.expectCell(0, 0, "Lecture / Pause");
  });

  it("resizes the grid, and refuses to cut buttons off", async () => {
    await app.session.click(await app.session.find(gridSize(5, 3)));
    await app.clickLabel(t("grid.more.cols"));
    await app.session.find(gridSize(6, 3));
    await app.session.find("[data-cell='0,5']");
    await app.clickLabel(t("grid.less.rows"));
    await app.session.find(withText(t("grid.wouldCut")));
    await app.shot("09-grid");
    await app.clickLabel(t("grid.less.cols"));
    await app.session.find(gridSize(5, 3));
    await app.session.chord(Key.Escape);
    await app.session.gone(withText(t("grid.cols")));
  });

  it("creates, duplicates, renames and deletes profiles", async () => {
    await app.session.click(await profileMenu());
    await app.clickText(t("profiles.create.action"), "button");
    await app.answerPrompt("Montage", t("common.create"));
    await app.session.find(withText(`${t("profiles.label")} Montage`, "button"));
    await app.expectCell(0, 0, app.emptyLabel(0, 0));

    await app.session.click(await profileMenu());
    await app.clickText(t("profiles.rename.action"), "button");
    await app.answerPrompt("Vidéo", t("common.rename"));
    await app.session.find(withText(`${t("profiles.label")} Vidéo`, "button"));

    await app.session.click(await profileMenu());
    await app.shot("10-profiles");
    await app.clickText("Démo", "button");
    await app.expectCell(0, 0, "Lecture / Pause");

    await app.session.click(await profileMenu());
    await app.clickText(t("profiles.duplicate.action"), "button");
    await app.answerPrompt("Démo bis", t("common.create"));
    await app.session.find(withText(`${t("profiles.label")} Démo bis`, "button"));
    await app.expectCell(0, 0, "Lecture / Pause");

    await app.session.click(await profileMenu());
    await app.clickText(t("profiles.delete.action"), "button");
    await app.clickText(t("profiles.delete.confirm"), "button");
    await app.session.gone(withText(`${t("profiles.label")} Démo bis`, "button"));

    const saved = await app.savedConfig();
    expect(saved.profiles.map((p) => p.name).sort()).toEqual(["Démo", "Vidéo"]);
  });

  it("keeps everything after closing the app", async () => {
    await app.session.click(await profileMenu());
    await app.clickText("Démo", "button");
    await app.expectCell(0, 1, "Mon terminal");
    // Typed just before closing: still within the save debounce.
    await app.select(0, 1);
    const input = await field(t("inspector.label"));
    await app.typeText(input, " !");
    await app.expectCell(0, 1, "Mon terminal !");
    await app.close();

    await app.open();
    await app.expectCell(0, 1, "Mon terminal !");
    await app.expectCell(1, 0, "Coller");
    await app.expectCell(2, 4, t("library.presets.cut.label"));
    await app.session.click(await profileMenu());
    await app.session.find(withText("Vidéo", "button"));
    await app.shot("11-reopened");
    await app.session.chord(Key.Escape);
  });

  it("shows the connections, a tablet and its presses", async () => {
    const pill = await app.session.find(`//header//button[@aria-pressed]`);
    await app.session.click(pill);
    await app.session.find(withText(t("app.backToEditor"), "button"));
    await app.session.find(withText(t("clients.empty")));
    await app.shot("12-connections-empty");

    const tablet = await app.connectTablet();
    expect(tablet.lastLayout()).toMatch(/^Layout "Démo": /);
    await app.session.waitUntil(
      async () => !(await app.session.text(pill)).includes(t("app.tablets").split(" | ")[0]!),
      "the pill should show the tablet",
    );
    await app.session.gone(withText(t("clients.empty")));

    // A press from the tablet is recorded, not executed.
    const before = (await app.actions()).length;
    tablet.press("copy");
    await app.session.waitUntil(
      async () => (await app.actions()).length === before + 1,
      "the press should be recorded",
    );
    expect((await app.actions()).at(-1)).toEqual({
      type: "hotkey",
      params: { keys: ["ControlLeft", "KeyC"] },
    });
    await app.session.find(withText(t("activity.ok")));
    await app.shot("13-connections-tablet");

    await app.clickText(t("app.backToEditor"), "button");
    await app.session.find("[role=grid]");
    await app.session.find(withText(t("editor.live")));
  });
});
