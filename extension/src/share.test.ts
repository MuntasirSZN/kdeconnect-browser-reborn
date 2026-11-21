import { describe, expect, test } from "bun:test";
import { MAX_IMAGE_BYTES, imageBytesAllowed, shareRequest, wantsImageData } from "./share";

describe("shareRequest", () => {
  test("carries a unique correlation id", () => {
    const first = shareRequest("dev1", "https://example.com");
    const second = shareRequest("dev1", "https://example.com");
    expect(typeof first.id).toBe("string");
    expect(first.id?.length).toBeGreaterThan(0);
    expect(first.id).not.toBe(second.id);
  });

  test("defaults imageData to null", () => {
    const msg = shareRequest("dev1", "https://example.com");
    expect(msg.data).toEqual({
      target: "dev1",
      url: "https://example.com",
      imageData: null,
    });
  });
});

describe("imageBytesAllowed", () => {
  test("rejects empty and oversized images", () => {
    expect(imageBytesAllowed(0)).toBe(false);
    expect(imageBytesAllowed(MAX_IMAGE_BYTES)).toBe(true);
    expect(imageBytesAllowed(MAX_IMAGE_BYTES + 1)).toBe(false);
  });

  test("stays under the host's 1 MiB frame limit after base64 expansion", () => {
    expect(Math.ceil((MAX_IMAGE_BYTES * 4) / 3)).toBeLessThan(1024 * 1024);
  });
});

describe("wantsImageData", () => {
  test("fetches image bytes only for unlinked image clicks", () => {
    expect(wantsImageData({ mediaType: "image", srcUrl: "https://x/i.png" })).toBe(true);
    expect(wantsImageData({ mediaType: "image", linkUrl: "https://x/page" })).toBe(false);
    expect(wantsImageData({ mediaType: "video", srcUrl: "https://x/i.mp4" })).toBe(false);
  });
});
