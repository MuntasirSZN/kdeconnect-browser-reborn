import type { Device } from "./generated/types.js";

/**
 * Device state shared by the popup and the options page, keyed by device ID.
 */

export const knownDevices: Record<string, Device> = {};

function replaceDevices(devices: Record<string, Device> | null | undefined): void {
  for (const key of Object.keys(knownDevices)) {
    delete knownDevices[key];
  }
  Object.assign(knownDevices, devices ?? {});
}

/** Add or replace a single device by ID. */
export function upsertDevice(device: Device): void {
  if (device?.id) {
    knownDevices[device.id] = device;
  }
}

/** Add or replace a single device and re-render, for `typeDeviceUpdate`. */
export function applyDevice(device: Device, writeDevices: () => void): void {
  upsertDevice(device);
  writeDevices();
}

/** Replace the snapshot and re-render, for `typeDevices`. */
export function applyDevices(
  devices: Record<string, Device> | null | undefined,
  writeDevices: () => void,
): void {
  replaceDevices(devices);
  writeDevices();
}

/**
 * Handle `typeDeviceUpdate`/`typeDevices` messages for both pages.
 * Returns false when the message is neither — callers then handle other types.
 */
export function applyMessage(
  msg: { type: unknown; data?: unknown },
  writeDevices: () => void,
): boolean {
  switch (msg.type) {
    case "typeDeviceUpdate":
      applyDevice(msg.data as Device, writeDevices);
      return true;
    case "typeDevices":
      applyDevices(msg.data as Record<string, Device> | null, writeDevices);
      return true;
    default:
      return false;
  }
}

/**
 * Clear `container` and append one element per device via `render`.
 * Null renders are skipped.
 */
export function renderDeviceList(
  container: HTMLElement,
  devices: Record<string, Device>,
  render: (device: Device) => HTMLElement | null,
): void {
  container.innerHTML = "";
  for (const device of Object.values(devices).filter(Boolean)) {
    const el = render(device);
    if (el) container.appendChild(el);
  }
}
