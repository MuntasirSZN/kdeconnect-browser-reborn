import { describe, expect, it } from "vitest";
import { clearDevices, getDevices, setDevices, updateDevice } from "@/device-cache";
import type { Device } from "@/generated/types";

const phone = { id: "dev1", name: "Pixel", type: "phone" } as Device;

describe("device cache", () => {
  it("replaces the snapshot", () => {
    setDevices({ dev1: phone });
    setDevices({ dev2: { ...phone, id: "dev2", name: "Tablet" } });
    expect(Object.keys(getDevices())).toEqual(["dev2"]);
  });
  it("upserts by id and clears", () => {
    setDevices({ dev1: phone });
    updateDevice({ ...phone, name: "Pixel 9" });
    expect(getDevices().dev1.name).toBe("Pixel 9");
    clearDevices();
    expect(getDevices()).toEqual({});
  });
});
