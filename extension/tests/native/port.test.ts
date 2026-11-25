/**
 * Integration tests for the native-messaging port (`native/port.ts`).
 *
 * A `FakePort` stands in for `connectNative`; a scripted `typeDevices` frame
 * *is* "a phone is connected".
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { browser } from "wxt/browser";
import { fakeBrowser } from "wxt/testing/fake-browser";

import { PROTOCOL_VERSION } from "../../src/generated/protocol";
import type { Device } from "../../src/generated/types";
import type { FakePort } from "../../src/native/testing/fake-port";
import type { ExtensionOptions, StatusData } from "../../src/shared-types";
import { DEFAULT_OPTIONS } from "../../src/storage";

vi.mock("../../src/menus/register", () => ({ rebuildContextMenus: vi.fn<() => void>() }));
vi.mock("../../src/messaging", () => ({
  emitDevices: vi.fn<(devices: Record<string, Device>) => void>(),
  emitStatus: vi.fn<(data: StatusData) => void>(),
  emitClearStatus: vi.fn<(key: string) => void>(),
}));
vi.mock("../../src/settings", () => ({ getOptions: vi.fn<() => ExtensionOptions>() }));

// All modules are imported dynamically after `vi.resetModules()` so each test
// gets a fresh port plus the device-cache and messaging instances the port
// actually uses.
let port: typeof import("../../src/native/port");
let dc: typeof import("../../src/device-cache");
let messaging: typeof import("../../src/messaging");
let settings: typeof import("../../src/settings");
let fake: import("../../src/native/testing/fake-port").FakePort;

const phone = { id: "dev1", name: "Pixel", type: "phone" } as Device;

function options(enabled: boolean): ReturnType<typeof settings.getOptions> {
  return { ...DEFAULT_OPTIONS, enabled };
}

function setRemoveAll(): void {
  vi.spyOn(browser.contextMenus, "removeAll").mockImplementation(() => undefined);
}

beforeEach(async () => {
  fakeBrowser.reset();
  vi.resetModules();
  setRemoveAll();
  // `connect()` writes a verbose diagnostic to console.debug.
  vi.spyOn(console, "debug").mockImplementation(() => {});

  fake = new (await import("../../src/native/testing/fake-port")).FakePort();
  settings = await import("../../src/settings");
  messaging = await import("../../src/messaging");
  dc = await import("../../src/device-cache");
  (browser.runtime as { connectNative: unknown }).connectNative = vi.fn<() => FakePort>(() => fake);
  vi.mocked(settings.getOptions).mockReturnValue(options(true));
  port = await import("../../src/native/port");
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ensureConnection", () => {
  it("posts typeVersion then typeDevices, once", () => {
    port.ensureConnection();
    port.ensureConnection();
    expect(browser.runtime.connectNative).toHaveBeenCalledTimes(1);
    expect(fake.posted[0]?.type).toBe("typeVersion");
    expect(fake.posted[1]?.type).toBe("typeDevices");
  });

  it("is a no-op when the integration is disabled", () => {
    vi.mocked(settings.getOptions).mockReturnValue(options(false));
    port.ensureConnection();
    expect(browser.runtime.connectNative).not.toHaveBeenCalled();
  });
});

describe("host replies", () => {
  it("typeVersion clears the connected badge", () => {
    port.connect();
    vi.mocked(messaging.emitClearStatus).mockClear();
    fake.emit({ type: "typeVersion", data: PROTOCOL_VERSION });
    expect(messaging.emitClearStatus).toHaveBeenCalledWith("connected");
  });

  it("typeDevices applies the snapshot and notifies views", () => {
    port.connect();
    const devices = { dev1: phone };
    fake.emit({ type: "typeDevices", data: devices });
    expect(dc.getDevices()).toEqual(devices);
    expect(messaging.emitDevices).toHaveBeenCalledWith(devices);
  });

  it("typeDeviceUpdate upserts one device and leaves the other keys", () => {
    port.connect();
    dc.setDevices({ dev1: phone, dev2: { ...phone, id: "dev2", name: "Tablet" } });
    fake.emit({
      type: "typeDeviceUpdate",
      data: { id: "dev1", name: "Pixel 9", type: "phone" },
    });
    expect(dc.getDevices().dev1?.name).toBe("Pixel 9");
    expect(dc.getDevices().dev2?.name).toBe("Tablet");
  });

  it("typeError publishes the host error status", () => {
    port.connect();
    fake.emit({ type: "typeError", data: "boom" });
    expect(messaging.emitStatus).toHaveBeenCalledWith({
      type: "typeError",
      key: "host",
      error: "boom",
    });
  });

  it("typeShareResult publishes success and failure", () => {
    port.connect();
    fake.emit({ type: "typeShareResult", data: { ok: true } });
    expect(messaging.emitStatus).toHaveBeenLastCalledWith({
      type: "typeShareResult",
      key: "share",
      message: "Sent",
    });
    fake.emit({ type: "typeShareResult", data: { ok: false, error: "no" } });
    expect(messaging.emitStatus).toHaveBeenLastCalledWith({
      type: "typeError",
      key: "share",
      error: "no",
    });
  });

  it("a mismatched typeVersion sets the badge and the status", () => {
    port.connect();
    fake.emit({ type: "typeVersion", data: "0.0.1" });
    expect(messaging.emitStatus).toHaveBeenCalledWith({
      type: "typeVersion",
      key: "update",
      update: PROTOCOL_VERSION,
      current: "0.0.1",
    });
  });
});

describe("disconnect", () => {
  it("clears devices and reconnects after the retry delay", async () => {
    vi.useFakeTimers();
    try {
      port.connect();
      fake.emit({ type: "typeVersion", data: PROTOCOL_VERSION }); // mark the handshake
      dc.setDevices({ dev1: phone });
      fake.disconnect();
      expect(dc.getDevices()).toEqual({});
      expect(browser.runtime.connectNative).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(1_000);
      expect(browser.runtime.connectNative).toHaveBeenCalledTimes(2);
      expect(messaging.emitStatus).toHaveBeenCalledWith({
        type: "typeError",
        key: "connected",
        error: "native host disconnected",
      });
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("postShare", () => {
  it("posts typeShare with the payload", () => {
    port.postShare({ target: "dev1", url: "https://example.com", imageData: null });
    const posted = fake.posted.find((m) => m.type === "typeShare");
    expect(posted?.data).toEqual({ target: "dev1", url: "https://example.com", imageData: null });
  });
});
