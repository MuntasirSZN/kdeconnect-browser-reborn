import { describe, expect, test } from "bun:test";
import type { Device, ExtensionOptions } from "./generated/types.js";
import { buildMenuSpec, parseMenuId, resolveMenuUrl } from "./menus.js";

const strings = {
  root: "KDE Connect",
  open: (name: string) => `Open on '${name}'`,
  call: (name: string) => `Call on '${name}'`,
};

const options: ExtensionOptions = {
  defaultOnly: false,
  defaultDeviceId: null,
  disableContextMenu: false,
};

function device(overrides: Partial<Device> = {}): Device {
  return {
    id: "dev1",
    type: "phone",
    name: "Pixel 9",
    iconName: "",
    statusIconName: "",
    isReachable: true,
    isTrusted: true,
    supportedPlugins: {},
    ...overrides,
  };
}

describe("buildMenuSpec", () => {
  test("empty when there are no devices", () => {
    expect(buildMenuSpec({}, options, strings)).toEqual([]);
  });

  test("empty when context menus are disabled", () => {
    expect(
      buildMenuSpec({ dev1: device() }, { ...options, disableContextMenu: true }, strings),
    ).toEqual([]);
  });

  test("one phone gets four top-level entries", () => {
    const spec = buildMenuSpec({ dev1: device() }, options, strings);
    expect(spec.map((e) => e.id)).toEqual([
      "open-link@dev1",
      "open-page@dev1",
      "call@dev1",
      "open-tab@dev1",
    ]);
    expect(spec.every((e) => e.parentId === undefined)).toBe(true);
    expect(spec[0].targetUrlPatterns).toEqual(["http://*/*", "https://*/*"]);
    expect(spec[2].targetUrlPatterns).toEqual(["tel:*"]);
  });

  test("one desktop gets no call entry", () => {
    const spec = buildMenuSpec({ dev1: device({ type: "desktop" }) }, options, strings);
    expect(spec.map((e) => e.id)).toEqual(["open-link@dev1", "open-page@dev1", "open-tab@dev1"]);
  });

  test("two devices share a root", () => {
    const spec = buildMenuSpec(
      { a: device({ id: "a" }), b: device({ id: "b" }) },
      options,
      strings,
    );
    expect(spec[0].id).toBe("kdeconnect-root");
    expect(spec.slice(1).every((e) => e.parentId === "kdeconnect-root")).toBe(true);
  });

  test("excludes unreachable and unpaired devices", () => {
    const spec = buildMenuSpec(
      {
        offline: device({ id: "offline", isReachable: false }),
        unpaired: device({ id: "unpaired", isTrusted: false }),
      },
      options,
      strings,
    );
    expect(spec).toEqual([]);
  });

  test("defaultOnly limits menus to the default device", () => {
    const spec = buildMenuSpec(
      { a: device({ id: "a" }), b: device({ id: "b" }) },
      { ...options, defaultOnly: true, defaultDeviceId: "b" },
      strings,
    );
    expect(new Set(spec.map((e) => e.id.split("@")[1]))).toEqual(new Set(["b"]));
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

  test("rejects unknown ids", () => {
    expect(parseMenuId("kdeconnect-root")).toBeNull();
    expect(parseMenuId("bogus@dev1")).toBeNull();
  });
});

describe("resolveMenuUrl", () => {
  test("uses the right source per action", () => {
    expect(resolveMenuUrl("open-link", { linkUrl: "https://example.com/a" })).toBe(
      "https://example.com/a",
    );
    expect(resolveMenuUrl("open-link", { srcUrl: "https://example.com/i.png" })).toBe(
      "https://example.com/i.png",
    );
    expect(resolveMenuUrl("open-page", { pageUrl: "https://example.com/p" })).toBe(
      "https://example.com/p",
    );
    expect(resolveMenuUrl("open-tab", {}, "https://example.com/t")).toBe("https://example.com/t");
    expect(resolveMenuUrl("call", { linkUrl: "tel:+15551234" })).toBe("tel:+15551234");
  });

  test("rejects non-shareable schemes", () => {
    expect(resolveMenuUrl("open-link", { linkUrl: "javascript:alert(1)" })).toBeNull();
    expect(resolveMenuUrl("open-page", { pageUrl: "file:///etc/passwd" })).toBeNull();
    expect(resolveMenuUrl("call", { linkUrl: "https://example.com" })).toBeNull();
  });

  test("returns null when no URL is present", () => {
    expect(resolveMenuUrl("open-link", {})).toBeNull();
  });
});
