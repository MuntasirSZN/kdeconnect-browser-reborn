/**
 * Background service worker state: device cache and options.
 *
 * Badge rendering lives in `browser-action.ts` and is re-exported here so
 * existing callers keep working.
 */

import type { Device } from "../generated/types";
import type { ExtensionOptions } from "../shared-types";

import { browser } from "wxt/browser";

export { clearBadge, setBadge } from "../browser-action";

let knownDevices: Record<string, Device> = {};
let options: ExtensionOptions = {
  enabled: true,
  defaultOnly: false,
  defaultDeviceId: null,
  disableContextMenu: false,
};

/** Current device snapshot keyed by device ID. */
export function getDevices(): Readonly<Record<string, Device>> {
  return knownDevices;
}

/** Replace the device snapshot (`typeDevices`). */
export function setDevices(devices: Record<string, Device> | null | undefined): void {
  knownDevices = devices ?? {};
}

/** Add or replace a single device by ID. */
export function updateDevice(device: Device): void {
  knownDevices[device.id] = device;
}

/** Drop cached devices and tell every view the list is empty. */
export function clearDevices(): void {
  knownDevices = {};
  broadcast({ type: "typeDevices", data: {} });
}

/** Current user options. */
export function getOptions(): Readonly<ExtensionOptions> {
  return options;
}

/** Replace the options object. */
export function setOptions(next: ExtensionOptions): void {
  options = next;
}

/** Update one options field in place. */
export function setOption<K extends keyof ExtensionOptions>(
  key: K,
  value: ExtensionOptions[K],
): void {
  options = { ...options, [key]: value };
}

/**
 * Broadcast a message to all extension views (popup, options).
 */
export function broadcast(msg: unknown): void {
  browser.runtime.sendMessage(msg).catch(() => {
    // Popup/options may be closed — ignore send errors.
  });
}
