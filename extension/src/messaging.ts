/**
 * Typed messaging between the background and extension views.
 */

import { defineExtensionMessaging } from "@webext-core/messaging";

import type { Device, ShareRequest } from "./generated/types";
import type { StatusData } from "./shared-types";

/** Background → view commands and view → background events. */
type Protocol = {
  connect(): void;
  getDevices(): Record<string, Device>;
  share(data: ShareRequest): void;
  devicesChanged(devices: Record<string, Device>): void;
  status(data: StatusData): void;
  clearStatus(key: string): void;
};

/** Typed messaging between the background and the extension views. */
export const { sendMessage, onMessage } = defineExtensionMessaging<Protocol>();

/** Emit the device snapshot to views; send failures are ignored. */
export function emitDevices(devices: Record<string, Device>): void {
  void sendMessage("devicesChanged", devices).catch(() => {
    // Views may have closed — ignore send errors.
  });
}

/** Emit a status line to views; send failures are ignored. */
export function emitStatus(data: StatusData): void {
  void sendMessage("status", data).catch(() => {});
}

/** Tell views to clear the status line for `key`; send failures are ignored. */
export function emitClearStatus(key: string): void {
  void sendMessage("clearStatus", key).catch(() => {});
}
