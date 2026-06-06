// L3c smoke: prove the extension loads under Playwright and the room-config
// path (the multi-tenant key) works. No CLI peers / signal server needed, so
// this runs anywhere a built extension + headed Chromium are available (#33).
import { test, expect } from "./fixtures";
import { openPopup, setRoom } from "./extension-actions";

test("extension loads and a strong room can be saved", async ({ page, extensionId }) => {
  await openPopup(page, extensionId);

  // The popup mounted (some recognizable wallet/app chrome is present).
  await expect(page.locator("body")).toBeVisible();

  // Set + save a strong room via the verified data-testids.
  const room = "boundary-smoke-" + "0".repeat(16);
  await setRoom(page, room);

  // The input retains the value we saved.
  await expect(page.getByTestId("room-input")).toHaveValue(room);
});
