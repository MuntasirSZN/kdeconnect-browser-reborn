/**
 * Popup view — message routing, sharing, and startup.
 */

import type { Device, Message } from "../generated/types";
import { applyMessage } from "../device-list";
import { shareRequest } from "../share";
import { renderEmpty, writeDevices } from "./render";
import { addStatus, removeStatus } from "./status";

let currentUrlPromise: Promise<string | null> | null = null;

function postMessage(msg: Message): void {
  chrome.runtime.sendMessage(msg).catch(() => {
    // background may be starting up
  });
}

/**
 * Resolve the active tab's URL once. Cached so the share path and the
 * default-device path share a single `tabs.query` call.
 */
function getCurrentTabUrl(): Promise<string | null> {
  currentUrlPromise ??= chrome.tabs
    .query({ active: true, currentWindow: true })
    .then((tabs) => tabs[0]?.url ?? null);
  return currentUrlPromise;
}

/** Share the active tab URL with `device`, then close the popup. */
function sendDevice(device: Device): void {
  void getCurrentTabUrl().then((url) => {
    if (url) {
      postMessage(shareRequest(device.id, url));
    }
    window.close();
  });
}

/** Route extension messages: status lines plus device-list updates. */
export function onMessage(msg: Record<string, unknown>): void {
  if (applyMessage(msg as { type: unknown; data?: unknown }, () => writeDevices(sendDevice))) {
    return;
  }

  switch (msg.type as string) {
    case "typeStatus":
      addStatus(msg as Message);
      break;
    case "typeClearStatus":
      removeStatus(msg as Message);
      break;
  }
}

/** Startup: show a loading placeholder, then request version and device updates. */
export async function init(): Promise<void> {
  renderEmpty("Loading…");

  const items = (await chrome.storage.sync.get({
    enabled: true,
    defaultOnly: false,
    defaultDeviceId: null,
  })) as { enabled?: boolean; defaultOnly?: boolean; defaultDeviceId?: string | null };

  if (items.enabled === false) {
    renderEmpty("KDE Connect is disabled in options.");
    return;
  }

  if (items.defaultOnly && items.defaultDeviceId) {
    const url = await getCurrentTabUrl();
    if (url) {
      postMessage({
        type: "typeShare",
        data: { target: items.defaultDeviceId, url },
      });
    }
    window.close();
    return;
  }

  await getCurrentTabUrl();
  postMessage({ type: "typeVersion" });
  postMessage({ type: "typeDevices" });
}
