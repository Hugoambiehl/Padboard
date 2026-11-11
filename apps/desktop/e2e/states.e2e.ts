// Button states on the tablet: simulated in test mode (state.json), checked
// on the fake tablet's output.
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, FakeTablet, literal, t } from "./harness";

let app: App;
let tablet: FakeTablet;
let micId = "";
beforeAll(async () => {
  app = await App.start();
  tablet = await app.connectTablet("e2e-states");
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const updates = () => tablet.lines.filter((l) => l.startsWith("Update "));
const audio = (inputMuted: boolean | null, outputMuted = false) => ({
  outputPercent: 50,
  outputMuted,
  inputMuted,
});

describe("button states", () => {
  it("a muted microphone is pushed to the tablet once", async () => {
    // A microphone button in the free cell.
    await app.select(2, 3);
    const mic = await app.session.find(
      `//aside//button[.//span[normalize-space(.)=${literal(t("library.presets.microphone.label"))}]]`,
    );
    await app.session.click(mic);
    await app.expectCell(2, 3, t("library.presets.microphone.label"));
    await app.session.waitUntil(async () => {
      const saved = JSON.parse(JSON.stringify(await app.savedConfig()));
      const button = saved.profiles[0].pages[0].buttons.find(
        (b: { action: { type: string } }) => b.action.type === "microphone",
      );
      micId = button?.id ?? "";
      return micId !== "";
    }, "the microphone button should be saved");

    await app.setState({ audio: audio(false) });
    await sleep(500);
    // Unmuted: the resting look, nothing to send.
    expect(updates()).toEqual([]);

    await app.setState({ audio: audio(true) });
    await tablet.waitFor(new RegExp(`^Update ${micId}: .* active$`));
    // The same state again (the sound server repeats itself): nothing more.
    await app.setState({ audio: audio(true), mediaPlaying: false });
    await sleep(500);
    expect(updates()).toHaveLength(1);
  });

  it("a tablet that connects gets the current states", async () => {
    const other = await app.connectTablet("e2e-states-2");
    await other.waitFor(new RegExp(`^State ${micId}: active=true unavailable=false$`));
  });

  it("a missing sound server makes audio buttons unavailable", async () => {
    const from = tablet.lines.length;
    await app.setState({ audio: "unavailable" });
    await tablet.waitFor(new RegExp(`^Update ${micId}: .*unavailable`), from);
    // The demo's volume buttons too, but not the shortcuts.
    await tablet.waitFor(/^Update volume-up: .*unavailable/, from);
    expect(tablet.lines.slice(from).some((l) => l.startsWith("Update copy"))).toBe(false);
  });
});
