import { expect, test } from "@playwright/test";

import { launch, type Harness } from "./harness";

let harness: Harness;

test.beforeAll(async () => {
  harness = await launch();
});

test.afterAll(async () => {
  await harness?.close();
});

test("disabling the integration empties the popup, re-enabling restores it", async () => {
  const options = await harness.openOptions();
  const enabled = options.locator("#enabled");

  await expect(enabled).toBeChecked();
  await enabled.uncheck();
  await options.locator("#save").click();
  await expect(options.locator("#status")).toContainText("Saved");

  const popup = await harness.openPopup();
  await expect(popup.locator("#devices")).toContainText("disabled in options");

  await enabled.check();
  await options.locator("#save").click();
  await expect(options.locator("#status")).toContainText("Saved");

  await popup.reload();
  await expect(popup.locator(".device", { hasText: "Pixel E2E" })).toBeVisible();
});
