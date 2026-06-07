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

// --- create wallet (DKG initiator) ------------------------------------------

/** Leave the Settings view (its "Done" button) so the wallet view is showing. */
export async function closeSettings(page: Page): Promise<void> {
  const done = page.getByRole("button", { name: /^done$/i });
  if (await done.count()) {
    await done.first().click();
  }
}

/**
 * Create a threshold wallet — the extension becomes the DKG initiator.
 *
 * Selectors verified against CreateWalletForm.svelte: stable ids `#cw-total`,
 * `#cw-threshold`, `#cw-curve`; both the main-view trigger and the form's submit
 * are labelled "Create wallet", but the main view unmounts when the form opens
 * (App.svelte `{#if showSettings}{:else if showCreateWallet}{:else}…`), so the
 * post-open lookup resolves to the submit. The DKG-creation form has no password
 * field (the keystore password is collected later), so `opts.password` is unused
 * here.
 */
export async function createWallet(
  page: Page,
  opts: { threshold: number; total: number; password: string; curve: string },
): Promise<void> {
  // `setRoom` leaves us on Settings; return to the wallet view.
  await closeSettings(page);

  // The main-view "Create wallet" button is `disabled` until the background
  // reports wsConnected (it enables once the post-room reconnect lands). Assert
  // it becomes enabled so a connection failure is a clear error, not a vague
  // click timeout.
  const trigger = page.getByRole("button", { name: /create wallet/i }).first();
  await trigger.waitFor({ state: "visible", timeout: 20_000 });
  await page.waitForFunction(
    () => {
      const b = Array.from(document.querySelectorAll("button")).find((el) =>
        /create wallet/i.test(el.textContent ?? ""),
      );
      return !!b && !(b as HTMLButtonElement).disabled;
    },
    { timeout: 30_000 },
  );
  await trigger.click();

  // CreateWalletForm overlay — fill the real inputs (defaults already match a
  // 2-of-3, but set them explicitly so the test controls the shape).
  await page.locator("#cw-total").fill(String(opts.total));
  await page.locator("#cw-threshold").fill(String(opts.threshold));
  if (opts.curve) {
    await page.locator("#cw-curve").selectOption(opts.curve).catch(() => {});
  }

  // Submit (form's button is also "Create wallet"; main view is unmounted now).
  await page.getByRole("button", { name: /^create wallet$/i }).click();
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
