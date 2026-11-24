import { beforeEach, describe, expect, it } from "vitest";
import { fakeBrowser } from "wxt/testing/fake-browser";

import { storage } from "#imports";

import { DEFAULT_OPTIONS, loadOptions, optionsItem } from "@/storage";

beforeEach(() => fakeBrowser.reset());

describe("loadOptions", () => {
  it("migrates legacy flat sync keys and removes them", async () => {
    await storage.setItem("sync:enabled", false);
    await storage.setItem("sync:defaultOnly", true);
    await storage.setItem("sync:defaultDeviceId", "dev1");
    await storage.setItem("sync:disableContextMenu", true);
    const expected = {
      enabled: false,
      defaultOnly: true,
      defaultDeviceId: "dev1",
      disableContextMenu: true,
    };
    expect(await loadOptions()).toEqual(expected);
    expect(await optionsItem.getValue()).toEqual(expected);
    expect(await storage.getItem("sync:enabled")).toBeNull();
    expect(await storage.getItem("sync:disableContextMenu")).toBeNull();
  });

  it("keeps an existing options object and ignores legacy keys", async () => {
    const existing = {
      enabled: false,
      defaultOnly: false,
      defaultDeviceId: null,
      disableContextMenu: false,
    };
    await optionsItem.setValue(existing);
    await storage.setItem("sync:enabled", true);
    expect(await loadOptions()).toEqual(existing);
  });

  it("returns defaults when nothing is stored", async () => {
    expect(await loadOptions()).toEqual(DEFAULT_OPTIONS);
  });
});
