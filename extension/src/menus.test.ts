import { describe, expect, test } from "bun:test";
import type { Device, ExtensionOptions } from "./generated/types.js";
import { buildMenuSpec, parseMenuId, resolveMenuUrl } from "./menus.js";

const strings = { open: (name: string) => `Open on '${name}'` };
const options: ExtensionOptions = {
  defaultOnly: false, defaultDeviceId: null, disableContextMenu: false,
};

function device(overrides: Partial<Device> = {}): Device {
  return {
    id: "dev1", type: "phone", name: "Pixel 9", iconName: "", statusIconName: "",
    isReachable: true, isTrusted: true, supportedPlugins: {}, ...overrides,
  };
}

describe("buildMenuSpec", () => {
  test("empty when there are no devices", () => {
    expect(buildMenuSpec({}, options, strings)).toEqual([]);
  });
  test("empty when context menus are disabled", () => {
    expect(buildMenuSpec({ dev1: device() }, { ...options, disableContextMenu: true }, strings)).toEqual([]);
  });
  test("one device gets flat link and page entries", () => {
    const spec = buildMenuSpec({ dev1: device() }, options, strings);
    expect(spec.map((e) => e.id)).toEqual(["open-link@dev1", "open-page@dev1"]);
    expect(spec[0].contexts).toEqual(["link", "image", "audio", "video"]);
    expect(spec[0].targetUrlPatterns).toEqual(["http://*/*", "https://*/*"]);
    expect(spec[1].contexts).toEqual(["page"]);
  });
  test("multiple devices get flat entries", () => {
    const spec = buildMenuSpec({ a: device({ id: "a" }), b: device({ id: "b" }) }, options, strings);
    expect(spec.map((e) => e.id)).toEqual(["open-link@a", "open-page@a", "open-link@b", "open-page@b"]);
  });
  test("tab sharing is Firefox-only and has device icons", () => {
    const chromeMenus = buildMenuSpec({ dev1: device() }, options, strings);
    const firefoxMenus = buildMenuSpec({ dev1: device() }, options, strings, true);
    expect(chromeMenus.map((e) => e.id)).toEqual(["open-link@dev1", "open-page@dev1"]);
    expect(firefoxMenus.map((e) => e.id)).toEqual(["open-link@dev1", "open-page@dev1", "open-tab@dev1"]);
    expect(firefoxMenus[0].icons).toEqual({ 16: "images/devices/phone.svg" });
  });
  test("excludes unreachable and unpaired devices", () => {
    const spec = buildMenuSpec({
      offline: device({ id: "offline", isReachable: false }),
      unpaired: device({ id: "unpaired", isTrusted: false }),
    }, options, strings);
    expect(spec).toEqual([]);
  });
  test("defaultOnly limits menus to the default device", () => {
    const spec = buildMenuSpec(
      { a: device({ id: "a" }), b: device({ id: "b" }) },
      { ...options, defaultOnly: true, defaultDeviceId: "b" }, strings,
    );
    expect(new Set(spec.map((e) => e.id.split("@")[1]))).toEqual(new Set(["b"]));
  });
  test("defaultOnly with no selected device does not expose all devices", () => {
    expect(buildMenuSpec({ a: device({ id: "a" }) }, { ...options, defaultOnly: true }, strings)).toEqual([]);
  });
  test("quotes in device name are preserved", () => {
    const spec = buildMenuSpec({ dev1: device({ name: `Pixel's <b>"test"` }) }, options, strings);
    expect(spec[0].title).toBe(`Open on 'Pixel's <b>"test"'`);
  });
});

describe("parseMenuId", () => {
  test("splits action and device id", () => {
    expect(parseMenuId("open-link@a1b2")).toEqual({ action: "open-link", deviceId: "a1b2" });
  });
  test("rejects removed or unknown ids", () => {
    expect(parseMenuId("kdeconnect-root")).toBeNull();
    expect(parseMenuId("call@dev1")).toBeNull();
    expect(parseMenuId("bogus@dev1")).toBeNull();
  });
});

describe("resolveMenuUrl", () => {
  test("uses the right source per action", () => {
    expect(resolveMenuUrl("open-link", { linkUrl: "https://example.com/a" })).toBe("https://example.com/a");
    expect(resolveMenuUrl("open-link", { srcUrl: "https://example.com/i.png" })).toBe("https://example.com/i.png");
    expect(resolveMenuUrl("open-page", { pageUrl: "https://example.com/p" })).toBe("https://example.com/p");
    expect(resolveMenuUrl("open-tab", { pageUrl: "https://example.com/t" })).toBe("https://example.com/t");
  });
  test("rejects non-web schemes", () => {
    expect(resolveMenuUrl("open-link", { linkUrl: "javascript:alert(1)" })).toBeNull();
    expect(resolveMenuUrl("open-page", { pageUrl: "file:///etc/passwd" })).toBeNull();
  });
  test("returns null when no URL is present", () => {
    expect(resolveMenuUrl("open-link", {})).toBeNull();
  });
});
