// Starts tauri-driver (which starts the app in test mode) and the fake
// tablet, and gives the tests a few helpers. Paths come from
// scripts/desktop-e2e.sh.
import { spawn, type ChildProcess } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import fr from "../src/locales/fr.json";
import { Session, type Element } from "./webdriver";

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set: run the tests with scripts/desktop-e2e.sh`);
  return value;
}

const APP = () => required("PADBOARD_E2E_APP");
const FAKE_TABLET = () => required("PADBOARD_E2E_FAKE_TABLET");
const SHOTS = () => required("PADBOARD_E2E_SHOTS");

/** A free TCP port on loopback. */
function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() =>
        typeof address === "object" && address ? resolve(address.port) : reject(),
      );
    });
  });
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// --- French texts --------------------------------------------------------------

/** The French UI text of `key`, with `{param}` replaced, as vue-i18n does. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  let value: unknown = fr;
  for (const part of key.split(".")) value = (value as Record<string, unknown>)[part];
  if (typeof value !== "string") throw new Error(`no French text for ${key}`);
  return value.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
}

/** XPath literal for `text`, quotes included. */
export function literal(text: string): string {
  if (!text.includes("'")) return `'${text}'`;
  if (!text.includes('"')) return `"${text}"`;
  return `concat('${text.split("'").join(`', "'", '`)}')`;
}

/** XPath: a `tag` whose whole visible text is `text`. */
export function withText(text: string, tag = "*"): string {
  return `//${tag}[normalize-space(.)=${literal(text)}]`;
}

// --- The app -------------------------------------------------------------------

export class App {
  /** Port of the app's USB (loopback) server, unique to this run. */
  readonly usbPort: number;
  session!: Session;
  private driver?: ChildProcess;
  private driverUrl = "";
  private tablets: FakeTablet[] = [];

  private constructor(
    /** The PADBOARD_TEST_DIR of this app: configuration and recorded actions. */
    readonly dir: string,
    usbPort: number,
  ) {
    this.usbPort = usbPort;
  }

  /** A fresh test directory, then tauri-driver, then the app. */
  static async start(): Promise<App> {
    const dir = await mkdtemp(join(tmpdir(), "padboard-e2e-"));
    const app = new App(dir, await freePort());
    try {
      await app.startDriver();
      await app.open();
    } catch (e) {
      await app.stop();
      throw e;
    }
    return app;
  }

  private async startDriver() {
    const port = await freePort();
    const nativePort = await freePort();
    const args = ["--port", String(port), "--native-port", String(nativePort)];
    if (process.env.PADBOARD_E2E_NATIVE_DRIVER) {
      args.push("--native-driver", process.env.PADBOARD_E2E_NATIVE_DRIVER);
    }
    // The app inherits this environment from tauri-driver.
    const env = {
      ...process.env,
      PADBOARD_TEST_DIR: this.dir,
      PADBOARD_PORT: String(this.usbPort),
      PADBOARD_LOG: process.env.PADBOARD_LOG ?? "warn,padboard_desktop_lib=info",
    };
    // The app logs to <test dir>/app.log (test mode).
    this.driver = spawn("tauri-driver", args, { env, stdio: ["ignore", "ignore", "inherit"] });
    this.driverUrl = `http://127.0.0.1:${port}`;
    const deadline = Date.now() + 10_000;
    for (;;) {
      try {
        const response = await fetch(`${this.driverUrl}/status`);
        if (response.ok) break;
      } catch {
        // not listening yet
      }
      if (Date.now() > deadline) throw new Error("tauri-driver did not start");
      await sleep(100);
    }
  }

  /** Starts the app window and waits for the editor. */
  async open(): Promise<void> {
    this.session = await Session.start(this.driverUrl, APP());
    await this.session.find("[role=grid] [data-cell='0,0']", 20_000);
  }

  /**
   * Closes the window the way the user does, and waits for the app to exit
   * (ending the WebDriver session would kill it instead).
   */
  async close(): Promise<void> {
    const exits = async () =>
      (await this.log()).split("\n").filter((l) => l.endsWith(" exited")).length;
    const before = await exits();
    await this.session
      .execute("window.__TAURI_INTERNALS__.invoke('plugin:window|close', { label: 'main' })")
      .catch(() => undefined);
    await this.session.waitUntil(
      async () => (await exits()) > before,
      "the app should exit",
      10_000,
    );
    await this.session.quit().catch(() => undefined);
  }

  /** The app's log (test mode writes it to the test directory). */
  log(): Promise<string> {
    return readFile(join(this.dir, "app.log"), "utf8").catch(() => "");
  }

  async stop(): Promise<void> {
    for (const tablet of this.tablets) tablet.stop();
    await this.session?.quit().catch(() => undefined);
    this.driver?.kill();
    if (!process.env.PADBOARD_E2E_KEEP) await rm(this.dir, { recursive: true, force: true });
  }

  /** Actions "executed" so far (test mode records them instead). */
  async actions(): Promise<{ type: string; params: { keys: string[] } }[]> {
    const text = await readFile(join(this.dir, "actions.jsonl"), "utf8").catch(() => "");
    return text
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  }

  /**
   * Simulates what the PC observes (test mode reads `state.json` instead of
   * the real microphone, sound server and players).
   */
  async setState(state: {
    audio?:
      { outputPercent: number; outputMuted: boolean; inputMuted: boolean | null } | "unavailable";
    mediaPlaying?: boolean;
    obs?:
      | { scene?: string; recording?: boolean; streaming?: boolean; mutedInputs?: string[] }
      | "unavailable";
  }): Promise<void> {
    await writeFile(join(this.dir, "state.json"), JSON.stringify(state));
  }

  /** The saved configuration (`profiles.json`). */
  async savedConfig(): Promise<{
    activeProfileId: string;
    profiles: {
      id: string;
      name: string;
      pages: { name: string; buttons: { label: string }[] }[];
    }[];
  }> {
    return JSON.parse(await readFile(join(this.dir, "config", "profiles.json"), "utf8"));
  }

  /** Screenshot of the window, in the screenshots directory. */
  async shot(name: string): Promise<void> {
    await mkdir(SHOTS(), { recursive: true });
    await sleep(250); // let the menu / hover animations settle
    await this.session.screenshot(join(SHOTS(), `${name}.png`));
  }

  /** Connects a fake tablet over "USB" (the app's loopback port). */
  async connectTablet(id = "e2e"): Promise<FakeTablet> {
    const tablet = new FakeTablet(FAKE_TABLET(), [
      "usb",
      "--port",
      String(this.usbPort),
      "--id",
      id,
    ]);
    this.tablets.push(tablet);
    await tablet.waitFor(/^Layout /);
    return tablet;
  }

  // --- Editor helpers ------------------------------------------------------

  cell(row: number, col: number): Promise<Element> {
    return this.session.find(`[role=grid] [data-cell='${row},${col}']`);
  }

  async cellLabel(row: number, col: number): Promise<string | null> {
    return this.session.attribute(await this.cell(row, col), "aria-label");
  }

  /** Waits until the cell's accessible label is `label`. */
  async expectCell(row: number, col: number, label: string): Promise<void> {
    await this.session.waitUntil(
      async () => (await this.cellLabel(row, col)) === label,
      `cell ${row},${col} should be ${JSON.stringify(label)}`,
    );
  }

  emptyLabel(row: number, col: number): string {
    return t("grid.emptyCell", { row: row + 1, col: col + 1 });
  }

  async select(row: number, col: number): Promise<void> {
    await this.session.click(await this.cell(row, col));
  }

  /** Clicks the displayed element whose text (or aria-label) is `text`. */
  async clickText(text: string, tag = "*"): Promise<void> {
    await this.session.click(await this.session.find(withText(text, tag)));
  }

  async clickLabel(label: string): Promise<void> {
    await this.session.click(await this.session.find(`[aria-label=${JSON.stringify(label)}]`));
  }

  /** Types into an input, one character at a time (see `fill`). */
  async typeText(input: Element, text: string): Promise<void> {
    for (const ch of text) await this.session.type(input, ch);
  }

  /**
   * Replaces the text of an input, then checks it (WebKitWebDriver can drop
   * keystrokes under load, notably on a virtual display).
   */
  async fill(input: Element, value: string): Promise<void> {
    for (let attempt = 0; ; attempt++) {
      await this.session.clear(input);
      // One character at a time: a whole string sent at once can lose
      // keystrokes while the page reacts to the first one.
      await this.typeText(input, value);
      const typed = await this.session.property<string>(input, "value");
      if (typed === value) return;
      if (attempt >= 2)
        throw new Error(`typed ${JSON.stringify(typed)} instead of ${JSON.stringify(value)}`);
      await sleep(200);
    }
  }

  /** Fills the open text prompt (profile / page name) and confirms. */
  async answerPrompt(value: string, confirm: string): Promise<void> {
    const input = await this.session.find("dialog[open] input");
    await this.fill(input, value);
    // Scoped to the dialog: a menu fading out behind it may have the same text.
    await this.session.click(
      await this.session.find(`//dialog[@open]//button[normalize-space(.)=${literal(confirm)}]`),
    );
    await this.session.gone("dialog[open]");
  }
}

// --- Fake tablet -----------------------------------------------------------------

/** The `fake-tablet` example, connected over USB, read line by line. */
export class FakeTablet {
  readonly lines: string[] = [];
  private readonly child: ChildProcess;

  constructor(binary: string, args: string[]) {
    this.child = spawn(binary, args, { stdio: ["pipe", "pipe", "inherit"] });
    let partial = "";
    this.child.stdout!.on("data", (chunk: Buffer) => {
      partial += chunk.toString("utf8");
      const parts = partial.split("\n");
      partial = parts.pop() ?? "";
      this.lines.push(...parts);
    });
  }

  /** Waits for a line matching `pattern` printed from index `from` on. */
  async waitFor(pattern: RegExp, from = 0, timeout = 10_000): Promise<string> {
    const deadline = Date.now() + timeout;
    for (;;) {
      const line = this.lines.slice(from).find((l) => pattern.test(l));
      if (line) return line;
      if (Date.now() > deadline) {
        throw new Error(`fake tablet: no line matching ${pattern}; got:\n${this.lines.join("\n")}`);
      }
      await sleep(100);
    }
  }

  /** The last layout received: `Layout "<profile>": <button ids>`. */
  lastLayout(): string | undefined {
    return this.lines.filter((l) => l.startsWith("Layout ")).at(-1);
  }

  press(buttonId: string): void {
    this.child.stdin!.write(`${buttonId}\n`);
  }

  stop(): void {
    this.child.kill();
  }
}
