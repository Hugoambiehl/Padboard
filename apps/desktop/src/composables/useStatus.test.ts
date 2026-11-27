import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import type { Snapshot } from "../types/status";

// The backend: `status` events, and `get_status` for the first snapshot.
let emit: ((event: { payload: unknown }) => void) | null = null;
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (_name: string, handler: (event: { payload: unknown }) => void) => {
    emit = handler;
    return () => undefined;
  },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: async () => base() }));

function base() {
  return {
    server: { appVersion: "0.4.0", usb: { state: "listening", port: 47321 } },
    usb: { adb: { state: "ready", path: "/usr/bin/adb" }, devices: [] },
    clients: [{ id: 1, deviceName: "Tab", connectedAtMs: 1000, tablet: { appVersion: "0.4.0" } }],
    pairedDevices: [{ deviceId: "tab", name: "Tab", pairedAtMs: 1, lastSeenMs: 2 }],
    activity: [],
  };
}

describe("useStatus", () => {
  beforeEach(() => {
    vi.resetModules();
    emit = null;
  });

  it("keeps what a reconnection did not change, so the other cards are not redrawn", async () => {
    const { useStatus } = await import("./useStatus");
    const { snapshot } = useStatus();
    await vi.waitFor(() => expect(snapshot.value).not.toBeNull());
    const before = snapshot.value!;
    const server = before.server;
    const usb = before.usb;
    const paired = before.pairedDevices;

    // The tablet's session replaced: a new client, everything else the same.
    const next = base();
    next.clients = [
      { id: 2, deviceName: "Tab", connectedAtMs: 2000, tablet: { appVersion: "0.4.0" } },
    ];
    emit!({ payload: next });
    await nextTick();

    const after = snapshot.value as Snapshot;
    expect(after.clients[0]?.id).toBe(2);
    expect(after.server).toBe(server);
    expect(after.usb).toBe(usb);
    expect(after.pairedDevices).toBe(paired);
    expect(after.clients).not.toBe(before.clients);
  });
});

describe("reconcile", () => {
  it("returns the previous value when nothing changed", async () => {
    const { reconcile } = await import("./reconcile");
    const prev = { a: [1, { b: 2 }], c: null };
    expect(reconcile(prev, JSON.parse(JSON.stringify(prev)))).toBe(prev);
  });

  it("shares unchanged items of a list that changed", async () => {
    const { reconcile } = await import("./reconcile");
    const prev = { list: [{ id: 1 }, { id: 2 }] };
    const out = reconcile(prev, { list: [{ id: 1 }, { id: 3 }, { id: 4 }] });
    expect(out.list).not.toBe(prev.list);
    expect(out.list[0]).toBe(prev.list[0]);
    expect(out.list.map((i) => i.id)).toEqual([1, 3, 4]);
    // A key removed is a change.
    const fewer = reconcile({ a: 1, b: 2 } as Record<string, number>, { a: 1 });
    expect(fewer).toEqual({ a: 1 });
  });
});
