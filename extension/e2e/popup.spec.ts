import { expect, test } from "@playwright/test";

import { launch, readHostLog, type Harness } from "./harness";

let harness: Harness;

test.beforeAll(async () => {
  harness = await launch();
});

test.afterAll(async () => {
  await harness?.close();
});

test("lists the scripted phone and shares the active tab to it", async () => {
  const page = await harness.openPopup();

  const device = page.locator(".device", { hasText: "Pixel E2E" });
  await expect(device).toBeVisible();
  await expect(device).toBeEnabled();

  await device.click();

  await expect
    .poll(() => readHostLog(harness.hostLog).find((frame) => frame.type === "typeShare"))
    .toBeTruthy();

  const share = readHostLog(harness.hostLog).find((frame) => frame.type === "typeShare") as {
    data?: { target?: string };
  };
  expect(share.data?.target).toBe("dev1");
});
