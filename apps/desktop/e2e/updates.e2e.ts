// The desktop update, end to end, against a local release server (the
// `test-updates` feature, built in by scripts/desktop-e2e.sh): the
// installer is downloaded, checked, saved in the test directory and shown,
// never started by Padboard while it is not signed (Bitdefender quarantined
// Padboard when it did). A failed automatic check shows nothing.
import { createServer, type Server } from "node:http";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { App, literal, t } from "./harness";

// Signed with a throwaway test key for version 1.2.3 (unit tests use them too).
const FIXTURES = fileURLToPath(
  new URL("../src-tauri/src/updates/fixtures/installer", import.meta.url),
);
const INSTALLER = "Padboard_1.2.3_x64-setup.exe";
const FILES = [INSTALLER, `${INSTALLER}.sig`, "SHA256SUMS", "SHA256SUMS.sig"];
const LIST = JSON.stringify([
  {
    tag_name: "v1.2.3",
    draft: false,
    prerelease: false,
    assets: [{ name: "latest.json" }, ...FILES.map((name) => ({ name }))],
  },
]);

/** GitHub's releases list answers 503 until this is set. */
let online = false;
let listRequests = 0;
let server: Server;
let base = "";
let app: App;

async function startServer(): Promise<void> {
  server = createServer((request, response) => {
    void (async () => {
      const path = (request.url ?? "").split("?")[0];
      if (path === "/api/releases") {
        listRequests += 1;
        if (!online) return response.writeHead(503).end();
        return response.writeHead(200, { "content-type": "application/json" }).end(LIST);
      }
      const file = FILES.find((name) => path === `/releases/download/v1.2.3/${name}`);
      if (!file) return response.writeHead(404).end();
      response.writeHead(200).end(await readFile(join(FIXTURES, file)));
    })();
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("no server address");
  base = `http://127.0.0.1:${address.port}`;
}

beforeAll(async () => {
  await startServer();
  app = await App.start("fr", {
    PADBOARD_TEST_RELEASES: base,
    PADBOARD_TEST_UPDATE_KEY: join(FIXTURES, "test-key.pub"),
    PADBOARD_TEST_APP_VERSION: "1.2.2",
  });
});
afterEach(async ({ task }) => {
  if (task.result?.state === "fail") await app?.shot(`FAILED ${task.name}`).catch(() => undefined);
});
afterAll(async () => {
  await app?.stop();
  server?.close();
});

const status = async () => app.session.text(await app.session.find("[data-updates-status]"));

describe("desktop updates", () => {
  it("a failed automatic check shows no error", async () => {
    await app.session.click(
      await app.session.find(`//button[@title=${literal(t("app.connectionsHint"))}]`),
    );
    await app.session.find("[data-updates]");
    // The first automatic check runs 10 s after launch.
    await app.session.waitUntil(
      async () => listRequests > 0,
      "the automatic check should ask for the releases",
      30_000,
    );
    await app.session.waitUntil(
      async () => (await app.log()).includes("automatic update check failed (not shown)"),
      "the failure should be logged",
    );
    expect(await status()).toBe(t("updates.idle"));
    expect(await app.log()).toContain("next update check in 5 min");
  });

  it("the update is found, downloaded, checked and shown, not started", async () => {
    online = true;
    await app.session.click(await app.session.find("[data-updates-check]"));
    await app.session.waitUntil(
      async () => (await status()) === t("updates.available", { version: "1.2.3" }),
      "the update should be offered",
    );
    const download = await app.session.find("[data-updates-install]");
    expect(await app.session.text(download)).toBe(t("updates.download"));
    await app.session.find(`//p[normalize-space(.)=${literal(t("updates.downloadHint"))}]`);
    // The header offers it too.
    await app.session.find(
      `//*[normalize-space(.)=${literal(t("updates.badge", { version: "1.2.3" }))}]`,
    );
    await app.shot("update-available");

    await app.session.click(download);
    await app.session.waitUntil(
      async () => (await status()) === t("updates.ready", { version: "1.2.3" }),
      "the installer should be ready",
      20_000,
    );
    const saved = join(app.dir, "config", "downloads", INSTALLER);
    expect(await app.session.text(await app.session.find("[data-updates-file]"))).toBe(saved);
    await app.session.find(`//p[normalize-space(.)=${literal(t("updates.readyHint"))}]`);
    // The file is the release's, saved whole, in the test directory.
    expect(await readFile(saved)).toEqual(await readFile(join(FIXTURES, INSTALLER)));
    const log = await app.log();
    expect(log).toContain("installer downloaded and checked");
    expect(log).toContain("the installer is not started by Padboard");
    expect(log).toContain("installer saved (not shown outside Windows)");
    expect(log).not.toContain("starting the signed installer");
    await app.shot("update-ready");
  });

  it("the file is shown again, and the release page opens", async () => {
    const shown = async () =>
      (await app.log()).split("\n").filter((l) => l.includes("not shown outside Windows")).length;
    const before = await shown();
    await app.session.click(await app.session.find("[data-updates-show]"));
    await app.session.waitUntil(async () => (await shown()) > before, "the file should be shown");

    await app.session.click(await app.session.find("[data-updates-page]"));
    await app.session.waitUntil(
      async () =>
        (await app.actions()).some(
          (a) =>
            a.type === "open" &&
            (a.params as unknown as { target: string }).target === `${base}/releases/tag/v1.2.3`,
        ),
      "the release page should open",
    );
    // Quitting is offered (not clicked: it would end the WebDriver session).
    await app.session.find("[data-updates-quit]");
  });
});
