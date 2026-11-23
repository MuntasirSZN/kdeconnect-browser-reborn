import type { Device } from "./generated/types";

let devices: Record<string, Device> = {};

/** Current device snapshot keyed by device ID. */
export function getDevices(): Readonly<Record<string, Device>> {
  return devices;
}

/** Replace the device snapshot (`typeDevices`). */
export function setDevices(next: Record<string, Device> | null | undefined): void {
  devices = next ?? {};
}

/** Add or replace a single device by ID. */
export function updateDevice(device: Device): void {
  devices[device.id] = device;
}

/** Drop all cached devices. */
export function clearDevices(): void {
  devices = {};
}
