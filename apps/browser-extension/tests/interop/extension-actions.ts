// Extension UI actions for the L3c harness (#33).
//
// All popup interactions live here so selectors are in ONE place. Two tiers:
//
//   VERIFIED  — uses data-testids that exist in the source today
//               (Settings room input/status; added in this change).
//   FIRST-RUN — create-wallet / sign steps depend on richer popup UI
//               (CreateWalletForm overlay etc.). The selectors below are the
//               best-effort starting point; confirm/adjust them on the first
//               headed run (`bun run test:interop --headed`) and keep them
//               centralized here.
import { type Page } from "@playwright/test";
import { popupUrl } from "./fixtures";

export async function openPopup(page: Page, extensionId: string): Promise<void> {
  await page.goto(popupUrl(extensionId));
  await page.waitForLoadState("domcontentloaded");
}

// --- VERIFIED: Settings → room (data-testids added to Settings.svelte) ------

/** Open Settings. The gear/settings control toggles `showSettings`. */
export async function openSettings(page: Page): Promise<void> {
  // Settings is reachable via a control with an accessible "Settings" name.
  // Fall back to clicking any element that reveals the room input.
  const gear = page.getByRole("button", { name: /settings/i });
  if (await gear.count()) {
    await gear.first().click();
  }
  await page.getByTestId("room-input").waitFor({ state: "visible", timeout: 10_000 });
}

/** Set + save the shared signal-server room (the multi-tenant key). */
export async function setRoom(page: Page, room: string): Promise<void> {
  await openSettings(page);
  const input = page.getByTestId("room-input");
  await input.fill(room);
  await page.getByRole("button", { name: /^save$/i }).click();
  // Assert the saved confirmation rather than guessing timing.
  await page.getByTestId("room-status").filter({ hasText: /saved/i }).waitFor({ timeout: 10_000 });
}

// --- FIRST-RUN: create wallet (DKG initiator) -------------------------------

/**
 * Create a threshold wallet — the extension becomes the DKG initiator.
 * Selectors here are the first-run starting point; confirm on a headed run.
 */
export async function createWallet(
  page: Page,
  opts: { threshold: number; total: number; password: string; curve: string },
): Promise<void> {
  await page.getByRole("button", { name: /create wallet/i }).first().click();
  // CreateWalletForm overlay — threshold/total/curve inputs + submit.
  // NEEDS-VERIFY: field names below are placeholders for the first headed run.
  const thr = page.getByLabel(/threshold/i);
  if (await thr.count()) await thr.fill(String(opts.threshold));
  const tot = page.getByLabel(/total|participants/i);
  if (await tot.count()) await tot.fill(String(opts.total));
  const pw = page.getByLabel(/password/i);
  if (await pw.count()) await pw.first().fill(opts.password);
  await page.getByRole("button", { name: /create|start|generate/i }).last().click();
}

/**
 * Sign a message with the active wallet (extension as initiator).
 * NEEDS-VERIFY selectors.
 */
export async function signMessage(page: Page, message: string): Promise<void> {
  await page.getByRole("button", { name: /sign message/i }).first().click();
  const box = page.getByPlaceholder(/message/i);
  if (await box.count()) await box.first().fill(message);
  await page.getByRole("button", { name: /^sign$|confirm/i }).last().click();
}
