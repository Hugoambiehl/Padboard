// Edited profile vs displayed profile: the profile menu only chooses what the
// editor shows; "Afficher sur la tablette" changes what the tablet shows.
// A fake tablet checks what is pushed.
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, FakeTablet, literal, t, withText } from "./harness";
import { Key } from "./webdriver";

let app: App;
let tablet: FakeTablet;
beforeAll(async () => {
  app = await App.start();
  tablet = await app.connectTablet("e2e-profiles");
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const profileButton = () => app.session.find(`//button[@aria-haspopup='menu']`);
const displayButton = withText(t("profiles.display"), "button");
/** The menu entry of a profile. */
const entry = (name: string) =>
  `//*[@role='menuitemradio'][.//span[normalize-space(.)=${literal(name)}]]`;
const layouts = () => tablet.lines.filter((l) => l.startsWith("Layout "));

/** Asserts the tablet received nothing new (pushes are throttled, saves debounced). */
async function expectNoPush(count: number) {
  await sleep(600);
  expect(layouts()).toHaveLength(count);
}

async function openMenu() {
  await app.session.click(await profileButton());
  await app.session.find("[role=menu]");
}

async function closeMenu() {
  await app.session.chord(Key.Escape);
  await app.session.gone("[role=menu]");
}

/** Whether the menu entry of `name` carries the "displayed" mark. */
async function markedDisplayed(name: string): Promise<boolean> {
  const marks = await app.session.findAll(
    `${entry(name)}//*[@aria-label=${literal(t("profiles.displayed"))}]`,
  );
  return marks.length > 0;
}

describe("edited and displayed profiles", () => {
  it("starts with the same profile edited and displayed", async () => {
    expect(tablet.lastLayout()).toMatch(/^Layout "Démo": /);
    await app.session.gone(displayButton);
    await app.session.find(withText(t("editor.live")));
  });

  it("opens a new profile in the editor only", async () => {
    const before = layouts().length;
    await openMenu();
    await app.clickText(t("profiles.create.action"), "button");
    await app.answerPrompt("Jeu", t("common.create"));
    await app.session.find(withText(`${t("profiles.label")} Jeu`, "button"));
    await app.session.find(displayButton);
    await app.session.find(withText(t("editor.notDisplayed")));

    // Editing it does not reach the tablet.
    await app.clickText(t("library.presets.copy.label"), "span");
    await app.expectCell(0, 0, t("library.presets.copy.label"));
    await expectNoPush(before);

    await openMenu();
    expect(await markedDisplayed("Démo")).toBe(true);
    expect(await markedDisplayed("Jeu")).toBe(false);
    expect(await app.session.attribute(await app.session.find(entry("Jeu")), "aria-checked")).toBe(
      "true",
    );
    await app.shot("20-editing-other-profile");
    await closeMenu();
  });

  it("shows the edited profile on the tablet on request", async () => {
    const before = layouts().length;
    await app.session.click(await app.session.find(displayButton));
    await tablet.waitFor(/^Layout "Jeu": /, before);
    await app.session.gone(displayButton);
    await app.session.find(withText(t("editor.live")));
    await openMenu();
    expect(await markedDisplayed("Jeu")).toBe(true);
    expect(await markedDisplayed("Démo")).toBe(false);
    await app.shot("21-displayed");

    // Back to editing "Démo": the tablet stays on "Jeu", even while editing.
    await app.session.click(await app.session.find(entry("Démo")));
    await app.expectCell(0, 1, "Copier");
    const count = layouts().length;
    await app.select(0, 1);
    const input = await app.session.find(
      `//label[span[normalize-space(.)=${literal(t("inspector.label"))}]]//input`,
    );
    await app.session.type(input, " tout");
    await app.expectCell(0, 1, "Copier tout");
    await expectNoPush(count);
    expect(tablet.lastLayout()).toMatch(/^Layout "Jeu": /);
  });

  it("moves the tablet to another profile when the displayed one is deleted", async () => {
    await openMenu();
    await app.session.click(await app.session.find(entry("Jeu")));
    await app.session.find(withText(`${t("profiles.label")} Jeu`, "button"));
    const before = layouts().length;

    await openMenu();
    await app.clickText(t("profiles.delete.action"), "button");
    // The confirmation says where the tablet goes.
    await app.session.find(
      withText(t("profiles.delete.confirmDisplayed", { name: "Démo" }), "button"),
    );
    await app.shot("22-delete-displayed");
    await app.clickText(t("profiles.delete.confirmDisplayed", { name: "Démo" }), "button");

    await tablet.waitFor(/^Layout "Démo": /, before);
    // The editor falls back to the displayed profile.
    await app.session.find(withText(`${t("profiles.label")} Démo`, "button"));
    await app.expectCell(0, 1, "Copier tout");
    await app.session.gone(displayButton);
    // Saved after the debounce.
    await app.session.waitUntil(async () => {
      const saved = await app.savedConfig();
      return saved.profiles.length === 1 && saved.profiles[0]!.id === saved.activeProfileId;
    }, "only Démo should be saved, displayed");
  });

  it("does not change the displayed profile when the edited one is deleted", async () => {
    await openMenu();
    await app.clickText(t("profiles.create.action"), "button");
    await app.answerPrompt("Brouillon", t("common.create"));
    await app.session.find(withText(`${t("profiles.label")} Brouillon`, "button"));
    const before = layouts().length;

    await openMenu();
    await app.clickText(t("profiles.delete.action"), "button");
    await app.clickText(t("profiles.delete.confirm"), "button");
    await app.session.find(withText(`${t("profiles.label")} Démo`, "button"));
    await expectNoPush(before);
  });
});
