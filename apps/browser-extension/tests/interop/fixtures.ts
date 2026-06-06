// Playwright fixtures for loading the built MV3 extension (L3c, #33).
//
// MV3 extensions can only be loaded into a *persistent* Chromium context with
// `--load-extension`, and only in headed mode (Chromium refuses to load
// unpacked extensions headless). We resolve the extension id from its
// background service worker.
import { test as base, chromium, type BrowserContext, type Worker } from "@playwright/test";
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const EXT_PATH = path.resolve(HERE, "../../.output/chrome-mv3");

export type ExtFixtures = {
  context: BrowserContext;
  extensionId: string;
};

export const test = base.extend<ExtFixtures>({
  // eslint-disable-next-line no-empty-pattern
  context: async ({}, use) => {
    if (!fs.existsSync(path.join(EXT_PATH, "manifest.json"))) {
      throw new Error(
        `Built extension not found at ${EXT_PATH}. Run \`bun run build\` first.`,
      );
    }
    // Use a system Chrome when PLAYWRIGHT_CHROME_PATH is set (e.g. Nix/CI where
    // Playwright's bundled chromium can't resolve system libs); else bundled.
    const executablePath = process.env.PLAYWRIGHT_CHROME_PATH || undefined;
    const context = await chromium.launchPersistentContext("", {
      headless: false,
      executablePath,
      args: [
        `--disable-extensions-except=${EXT_PATH}`,
        `--load-extension=${EXT_PATH}`,
        "--no-first-run",
        "--no-sandbox",
      ],
    });
    await use(context);
    await context.close();
  },

  extensionId: async ({ context }, use) => {
    // The service worker URL is chrome-extension://<id>/background.js
    let sw: Worker | undefined = context.serviceWorkers()[0];
    if (!sw) sw = await context.waitForEvent("serviceworker", { timeout: 30_000 });
    const id = new URL(sw.url()).host;
    await use(id);
  },
});

export const expect = test.expect;

/** URL of the extension popup page. */
export function popupUrl(extensionId: string): string {
  return `chrome-extension://${extensionId}/popup.html`;
}
