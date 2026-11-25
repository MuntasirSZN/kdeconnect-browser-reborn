/**
 * Popup view — device sharing, status routing, and startup.
 */

import type { Device } from "../../generated/types";
import { onMessage, sendMessage } from "../../messaging";
import { renderEmpty, writeDevices } from "../../popup/render";
import { addStatus, removeStatus } from "../../popup/status";
import { loadOptions } from "../../storage";
import { getCurrentTabUrl } from "../../tabs";

let currentUrlPromise: Promise<string | null> | null = null;

/** Resolve the active tab's URL once; later shares reuse it. */
function resolveCurrentUrl(): Promise<string | null> {
  currentUrlPromise ??= getCurrentTabUrl();
  return currentUrlPromise;
}

/** Share the active tab URL with `device`, then close the popup. */
function sendDevice(device: Device): void {
  void resolveCurrentUrl().then((url) => {
    if (url) void sendMessage("share", { target: device.id, url, imageData: null });
    window.close();
  });
}

/** Startup: show a loading placeholder, then request devices. */
export async function init(): Promise<void> {
  renderEmpty("Loading…");

  const opts = await loadOptions();
  if (!opts.enabled) {
    renderEmpty("KDE Connect is disabled in options.");
    return;
  }

  if (opts.defaultOnly && opts.defaultDeviceId) {
    const url = await resolveCurrentUrl();
    if (url) void sendMessage("share", { target: opts.defaultDeviceId, url, imageData: null });
    window.close();
    return;
  }

  await resolveCurrentUrl();
  void sendMessage("connect");
  writeDevices(await sendMessage("getDevices"), sendDevice);
}

onMessage("devicesChanged", ({ data }) => writeDevices(data, sendDevice));
onMessage("status", ({ data }) => addStatus(data));
onMessage("clearStatus", ({ data }) => removeStatus(data));

document.addEventListener("DOMContentLoaded", () => {
  init().catch((e: unknown) => console.warn("popup init failed", e));
});
