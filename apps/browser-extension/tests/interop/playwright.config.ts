import { defineConfig } from "@playwright/test";

// L3c interop harness (#33). MV3 extension loading requires headed Chromium and
// a persistent context (see fixtures.ts), so we force a single worker and no
// retries — these are real ceremonies over a live signal server, not unit tests.
export default defineConfig({
  testDir: ".",
  testMatch: "**/*.pw.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 120_000,
  reporter: [["list"]],
  use: {
    headless: false,
    trace: "retain-on-failure",
  },
});
