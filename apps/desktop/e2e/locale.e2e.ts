// The UI and the texts sent to the tablet follow the PC's language: English here.
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, FakeTablet, literal } from "./harness";
import en from "../src/locales/en.json";

let app: App;
let tablet: FakeTablet;
beforeAll(async () => {
  app = await App.start("en");
  tablet = await app.connectTablet("e2e-locale");
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
});

describe("English", () => {
  it("the editor is in English", async () => {
    await app.session.find(`//*[normalize-space(.)=${literal(en.library.title)}]`);
    await app.session.find(`//*[normalize-space(.)=${literal(en.inspector.noneTitle)}]`);
    expect(await app.session.execute<string>("return document.documentElement.lang")).toBe("en");
    await app.shot("english-editor");
  });

  it("the demo profile and the tablet texts are in English", async () => {
    await tablet.waitFor(/^Layout /);
    const layout = tablet.lines.find((l) => l.startsWith("Layout ")) ?? "";
    expect(layout).toContain("Demo");
    // The mute button, once the sound is muted: "Muted" from the PC.
    await app.setState({ audio: { outputPercent: 50, outputMuted: true, inputMuted: false } });
    await tablet.waitFor(/^Update mute: "Mute" .*\(Muted\)/);
  });
});
