// End-to-end tests: drive the real window through tauri-driver. Local only,
// run them with scripts/desktop-e2e.sh (it builds the app and sets the paths).
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["e2e/**/*.e2e.ts"],
    environment: "node",
    // One app window at a time; steps of a file run in order.
    fileParallelism: false,
    testTimeout: 60_000,
    hookTimeout: 60_000,
    // A failed step leaves the app in an unknown state: stop the file there.
    bail: 1,
  },
});
