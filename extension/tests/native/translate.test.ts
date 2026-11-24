import { describe, expect, it } from "vitest";

import type { Device, Message } from "../../src/generated/types";
import { translateHostMessage } from "../../src/native/translate";

function msg(type: Message["type"], data?: unknown): Message {
  return { type, data };
}

const phone = { id: "dev1", name: "Pixel", type: "phone" } as Device;

describe("translateHostMessage", () => {
  it("typeDevices → one devices effect", () => {
    expect(translateHostMessage(msg("typeDevices", { dev1: phone }), "1.0.0")).toEqual([
      { kind: "devices", devices: { dev1: phone } },
    ]);
  });
  it("typeDeviceUpdate → one device effect", () => {
    expect(translateHostMessage(msg("typeDeviceUpdate", phone), "1.0.0")).toEqual([
      { kind: "device", device: phone },
    ]);
  });
  it("typeError → error effect", () => {
    expect(translateHostMessage(msg("typeError", "boom"), "1.0.0")).toEqual([
      { kind: "error", key: "host", error: "boom" },
    ]);
  });
  it("typeShareResult ok and not-ok", () => {
    expect(translateHostMessage(msg("typeShareResult", { ok: true }), "1.0.0")).toEqual([
      { kind: "shareResult", ok: true },
    ]);
    expect(
      translateHostMessage(msg("typeShareResult", { ok: false, error: "nope" }), "1.0.0"),
    ).toEqual([{ kind: "shareResult", ok: false, error: "nope" }]);
  });
  it("typeVersion equal and different", () => {
    expect(translateHostMessage(msg("typeVersion", "1.0.0"), "1.0.0")).toEqual([
      { kind: "version", remote: "1.0.0", matches: true },
    ]);
    expect(translateHostMessage(msg("typeVersion", "0.1.4"), "1.0.0")).toEqual([
      { kind: "version", remote: "0.1.4", matches: false },
    ]);
  });
  it("unknown type → []", () => {
    expect(translateHostMessage({ type: "typeShare" }, "1.0.0")).toEqual([]);
  });
});
