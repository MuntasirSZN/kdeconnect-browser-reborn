import { expect, test, type Page } from "@playwright/test";

import { launch, type Harness } from "./harness";

test.describe.configure({ mode: "serial" });

let harness: Harness;

test.beforeAll(async () => {
  harness = await launch({ mode: "error" });
});

test.afterAll(async () => {
  await harness?.close();
});

async function badgeText(page: Page): Promise<string> {
  return page.evaluate(() => chrome.action.getBadgeText({}));
}

test("shows the host error, flags the badge, and recovers", async () => {
  const popup = await harness.openPopup();

  await expect(popup.locator("#status-host")).toContainText("kdeconnectd not running");
  await expect.poll(() => badgeText(popup)).toBe("!");

  harness.setMode("ok");
  await popup.reload();

  await expect(popup.locator(".device", { hasText: "Pixel E2E" })).toBeVisible();
  await expect.poll(() => badgeText(popup)).toBe("");
});

test("flags a protocol mismatch", async () => {
  // A fresh context: the first test already completed its handshake, and a
  // version check only happens when a port is newly opened.
  const mismatch = await launch({ mode: "mismatch" });
  try {
    const popup = await mismatch.openPopup();

    await expect(popup.locator("#status-update")).toContainText("Host upgrade v1.0.0 available");
    await expect.poll(() => badgeText(popup)).toBe("!");
  } finally {
    await mismatch.close();
  }
});
