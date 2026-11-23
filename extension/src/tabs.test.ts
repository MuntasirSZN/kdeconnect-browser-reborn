import { afterEach, describe, expect, it, vi } from "vitest";

import { browser } from "wxt/browser";
import { getCurrentTabUrl } from "./tabs";

const queryMock = vi.spyOn(browser.tabs, "query");

afterEach(() => queryMock.mockReset());

describe("getCurrentTabUrl", () => {
  it("returns the URL of the active tab", async () => {
    queryMock.mockResolvedValue([{ id: 1, url: "https://example.com" }] as never);
    expect(await getCurrentTabUrl()).toBe("https://example.com");
  });

  it("returns null when there is no active tab", async () => {
    queryMock.mockResolvedValue([] as never);
    expect(await getCurrentTabUrl()).toBeNull();
  });

  it("returns null when the active tab has no URL", async () => {
    queryMock.mockResolvedValue([{ id: 1 }] as never);
    expect(await getCurrentTabUrl()).toBeNull();
  });
});
