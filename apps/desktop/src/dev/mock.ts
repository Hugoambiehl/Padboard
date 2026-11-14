// Development-only preview without the Rust backend: `npm run dev`, then open
// http://localhost:1420/?mock in a browser (or a headless one for
// screenshots). Never loaded by the Tauri app or the production build.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { EditorState } from "../generated/types";
import type { Snapshot } from "../types/status";
import fixture from "./fixture-config.json";

const editor: EditorState = {
  config: fixture as EditorState["config"],
  canUndo: true,
  canRedo: false,
};

const status: Snapshot = {
  server: {
    name: "PC-HUGO",
    usb: { state: "listening", port: 47321, address: "127.0.0.1:47321", errorDetail: null },
    wifi: {
      enabled: true,
      forced: null,
      forcedByTestMode: false,
      listener: { state: "listening", port: 47322, address: "0.0.0.0:47322", errorDetail: null },
      lanAddresses: [
        { address: "192.168.1.42", interface: "wlan0", physical: true },
        { address: "172.18.0.1", interface: "br-568a00b37a31", physical: false },
      ],
      fingerprint: "44QK-JFGX-DRZ6",
      tlsError: null,
      mdns: { state: "announced" },
    },
    invalidSettings: [],
  },
  clients: [
    {
      id: 1,
      deviceId: "tab",
      deviceName: "Galaxy Tab A",
      transport: "usb",
      peer: "127.0.0.1:50000",
      connectedAtMs: Date.now(),
      kiosk: true,
    },
  ],
  pairing: { state: "idle" },
  pairedDevices: [],
  usb: {
    adb: { state: "ready", path: "/usr/bin/adb", version: "1.0.41" },
    devices: [
      {
        serial: "R52N10ABCDE",
        model: "SM-T580",
        state: { state: "ready" },
        kiosk: { state: "off" },
      },
    ],
  },
  activity: [],
};

const keys = [
  "ControlLeft",
  "ShiftLeft",
  "AltLeft",
  "MetaLeft",
  "KeyA",
  "KeyC",
  "KeyV",
  "KeyT",
  "KeyZ",
  "F13",
  "PrintScreen",
  "MediaPlayPause",
  "AudioVolumeUp",
  "AudioVolumeDown",
];

const hk = (...keys: string[]) => ({ type: "hotkey", params: { keys } });
const presets = [
  { id: "hotkey", group: "system", color: "grey", icon: "Keyboard", action: hk() },
  { id: "copy", group: "system", color: "blue", icon: "Copy", action: hk("ControlLeft", "KeyC") },
  {
    id: "paste",
    group: "system",
    color: "blue",
    icon: "ClipboardPaste",
    action: hk("ControlLeft", "KeyV"),
  },
  { id: "screenshot", group: "system", color: "teal", icon: "Camera", action: hk("PrintScreen") },
  { id: "lock", group: "system", color: "red", icon: "Lock", action: hk("MetaLeft", "KeyL") },
  { id: "playPause", group: "media", color: "orange", icon: "Play", action: hk("MediaPlayPause") },
  { id: "volumeUp", group: "media", color: "violet", icon: "Volume2", action: hk("AudioVolumeUp") },
  { id: "mute", group: "media", color: "violet", icon: "VolumeX", action: hk("AudioVolumeMute") },
];

/** `&update=available|linux|downloading|failed` previews the updates card. */
function updates() {
  const preview = new URLSearchParams(location.search).get("update");
  const status =
    preview === "available"
      ? { state: "available", version: "0.2.0", installable: true }
      : preview === "linux"
        ? { state: "available", version: "0.2.0", installable: false }
        : preview === "downloading"
          ? { state: "downloading", version: "0.2.0", received: 6_000_000, total: 9_000_000 }
          : preview === "failed"
            ? { state: "failed", error: "signature" }
            : { state: "upToDate", checkedAt: Date.now() };
  return { current: "0.1.0", preferences: { check: true, prereleases: false }, status };
}

export function install() {
  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "get_status":
          return status;
        case "get_editor":
          return editor;
        case "list_keys":
          return keys;
        case "library":
          return presets;
        case "edit":
          return {};
        case "undo":
        case "redo":
          return false;
        case "get_obs":
          return {
            available: true,
            enabled: true,
            host: "127.0.0.1",
            port: 4455,
            hasPassword: true,
            status: { state: "connected", version: "5.7.3" },
            scenes: ["Caméra", "Jeu", "Pause"],
            inputs: ["Mic/Aux", "Desktop Audio"],
          };
        case "set_obs":
          return null;
        case "get_autostart":
          return false;
        case "get_screensaver":
          return { enabled: true, delaySeconds: 300 };
        case "test_button":
          return { status: "ok" };
        case "get_locale":
          return new URLSearchParams(location.search).get("lang") === "en" ? "en" : "fr";
        case "plugin:app|version":
          return "0.1.0";
        case "get_updates":
          return updates();
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
