// Minimal W3C WebDriver client (https://www.w3.org/TR/webdriver2/), enough to
// drive the Tauri window through tauri-driver. Written by hand to avoid a
// large dependency for local-only tests.
import { writeFile } from "node:fs/promises";

const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

export interface Element {
  [ELEMENT]: string;
}

export class WebDriverError extends Error {
  constructor(
    readonly error: string,
    message: string,
  ) {
    super(`${error}: ${message}`);
  }
}

/** Key values of the WebDriver "keys" table (normalized key → code point). */
export const Key = {
  Enter: "",
  Escape: "",
  Backspace: "",
  Delete: "",
  Tab: "",
  ArrowLeft: "",
  ArrowUp: "",
  ArrowRight: "",
  ArrowDown: "",
  Shift: "",
  Control: "",
  Alt: "",
} as const;

async function request<T>(method: string, url: string, body?: unknown): Promise<T> {
  const response = await fetch(url, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = (await response.json()) as {
    value: T | { error: string; message: string };
  };
  const value = json.value;
  if (!response.ok) {
    const e = value as { error?: string; message?: string };
    throw new WebDriverError(e.error ?? String(response.status), e.message ?? "");
  }
  return value as T;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class Session {
  private constructor(private readonly base: string) {}

  /** Opens a session: tauri-driver starts the application. */
  static async start(driver: string, application: string): Promise<Session> {
    const value = await request<{ sessionId: string }>("POST", `${driver}/session`, {
      capabilities: { alwaysMatch: { "tauri:options": { application } } },
    });
    return new Session(`${driver}/session/${value.sessionId}`);
  }

  /** Closes the window and stops the application. */
  async quit(): Promise<void> {
    await request("DELETE", this.base);
  }

  /** Closes the current window, as the user would (the session then ends). */
  async closeWindow(): Promise<void> {
    await request("DELETE", `${this.base}/window`);
  }

  private call<T>(method: string, path: string, body?: unknown) {
    return request<T>(method, `${this.base}${path}`, body);
  }

  // --- Finding -------------------------------------------------------------

  async findAll(selector: string, root?: Element): Promise<Element[]> {
    const using = selector.startsWith("/") || selector.startsWith("(") ? "xpath" : "css selector";
    const path = root ? `/element/${root[ELEMENT]}/elements` : "/elements";
    return this.call<Element[]>("POST", path, { using, value: selector });
  }

  /**
   * Waits until `selector` (CSS, or XPath when it starts with `/` or `(`)
   * matches a displayed element, and returns the first one.
   */
  async find(selector: string, timeout = 5000): Promise<Element> {
    let found: Element | undefined;
    await this.waitUntil(
      async () => {
        for (const element of await this.findAll(selector)) {
          if (await this.displayed(element)) {
            found = element;
            return true;
          }
        }
        return false;
      },
      `no displayed element for ${selector}`,
      timeout,
    );
    return found!;
  }

  /** Waits until no displayed element matches `selector`. */
  async gone(selector: string, timeout = 5000): Promise<void> {
    await this.waitUntil(
      async () => {
        for (const element of await this.findAll(selector)) {
          if (await this.displayed(element).catch(() => false)) return false;
        }
        return true;
      },
      `still displayed: ${selector}`,
      timeout,
    );
  }

  async waitUntil(
    condition: () => Promise<boolean>,
    message: string,
    timeout = 5000,
  ): Promise<void> {
    const deadline = Date.now() + timeout;
    let last: unknown;
    for (;;) {
      try {
        if (await condition()) return;
      } catch (e) {
        // Stale elements while Vue re-renders: try again.
        last = e;
      }
      if (Date.now() > deadline) {
        throw new Error(`${message} (after ${timeout} ms)${last ? `; last error: ${last}` : ""}`);
      }
      await sleep(100);
    }
  }

  // --- Reading -------------------------------------------------------------

  text(element: Element): Promise<string> {
    return this.call<string>("GET", `/element/${element[ELEMENT]}/text`);
  }

  attribute(element: Element, name: string): Promise<string | null> {
    return this.call<string | null>("GET", `/element/${element[ELEMENT]}/attribute/${name}`);
  }

  property<T = unknown>(element: Element, name: string): Promise<T> {
    return this.call<T>("GET", `/element/${element[ELEMENT]}/property/${name}`);
  }

  displayed(element: Element): Promise<boolean> {
    return this.call<boolean>("GET", `/element/${element[ELEMENT]}/displayed`);
  }

  enabled(element: Element): Promise<boolean> {
    return this.call<boolean>("GET", `/element/${element[ELEMENT]}/enabled`);
  }

  /** Runs `script` (a function body) in the page. */
  execute<T>(script: string, ...args: unknown[]): Promise<T> {
    return this.call<T>("POST", "/execute/sync", { script, args });
  }

  /** Same as `execute`, for an async function body (resolved value returned). */
  executeAsync<T>(script: string, ...args: unknown[]): Promise<T> {
    return this.call<T>("POST", "/execute/async", {
      script: `const done = arguments[arguments.length - 1];
        (async (...args) => { ${script} })(...Array.from(arguments).slice(0, -1))
          .then(done, (e) => done({ __error: String(e) }));`,
      args,
    });
  }

  // --- Acting --------------------------------------------------------------

  click(element: Element): Promise<void> {
    return this.call("POST", `/element/${element[ELEMENT]}/click`, {});
  }

  clear(element: Element): Promise<void> {
    return this.call("POST", `/element/${element[ELEMENT]}/clear`, {});
  }

  /** Types into an element (focuses it first). */
  type(element: Element, text: string): Promise<void> {
    return this.call("POST", `/element/${element[ELEMENT]}/value`, { text });
  }

  /**
   * Presses keys on whatever has focus: `keys` are held in order then
   * released in reverse (a chord), e.g. `chord(Key.Control, "z")`.
   */
  async chord(...keys: string[]): Promise<void> {
    const actions = [
      ...keys.map((value) => ({ type: "keyDown", value })),
      ...[...keys].reverse().map((value) => ({ type: "keyUp", value })),
    ];
    await this.call("POST", "/actions", { actions: [{ type: "key", id: "keyboard", actions }] });
    await this.call("DELETE", "/actions");
  }

  /** Presses and releases each key in turn. */
  async press(...keys: string[]): Promise<void> {
    for (const key of keys) await this.chord(key);
  }

  /**
   * Drags with the mouse from the center of `from` to the center of `to`,
   * through a few intermediate points (the editor starts a drag after a few
   * pixels and tracks the hovered cell on every move).
   */
  async drag(from: Element, to: Element): Promise<void> {
    const origin = (element: Element) => ({ [ELEMENT]: element[ELEMENT] });
    const actions = [
      { type: "pointerMove", origin: origin(from), x: 0, y: 0, duration: 0 },
      { type: "pointerDown", button: 0 },
      { type: "pointerMove", origin: "pointer", x: 8, y: 8, duration: 50 },
      { type: "pointerMove", origin: origin(to), x: -6, y: -6, duration: 200 },
      { type: "pointerMove", origin: origin(to), x: 0, y: 0, duration: 50 },
      { type: "pause", duration: 50 },
      { type: "pointerUp", button: 0 },
    ];
    await this.call("POST", "/actions", {
      actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions }],
    });
    await this.call("DELETE", "/actions");
  }

  /** Screenshot of the window, saved as PNG. */
  async screenshot(path: string): Promise<void> {
    const base64 = await this.call<string>("GET", "/screenshot");
    await writeFile(path, Buffer.from(base64, "base64"));
  }
}
