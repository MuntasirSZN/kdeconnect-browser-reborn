/**
 * Native-messaging port lifecycle and request routing.
 */

import { browser } from "wxt/browser";
import type { Browser } from "wxt/browser";

import { clearBadge, setBadge } from "../browser-action";
import { clearDevices, setDevices, updateDevice } from "../device-cache";
import { HOST_NAME, PROTOCOL_VERSION } from "../generated/protocol";
import type { Message, ShareRequest } from "../generated/types";
import { rebuildContextMenus } from "../menus/register";
import { parseMenuId, resolveMenuUrl } from "../menus/spec";
import { emitClearStatus, emitDevices, emitStatus } from "../messaging";
import { createRetryState, markHandshake, nextRetryDelay, resetRetry } from "../reconnect";
import type { RetryState } from "../reconnect";
import { getOptions } from "../settings";
import { shareRequest, wantsImageData, imageBytesAllowed, MAX_IMAGE_BYTES } from "../share";
import { translateHostMessage } from "./translate";
import type { HostEffect } from "./translate";

let port: Browser.runtime.Port | null = null;
let retry: RetryState = createRetryState();
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

/** Firefox sets `port.error` on disconnect instead of `lastError`. */
interface FirefoxPort {
  error?: { message?: string };
}

/** Connect unless already connected or disabled; user actions reset the backoff budget. */
export function connect(isUserAction = false): void {
  if (port || !getOptions().enabled) return;

  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  retry.handshakeAt = null;
  if (isUserAction) resetRetry(retry);

  clearBadge("connected");
  emitClearStatus("connected");

  // Diagnostic: verbose-only, so normal consoles stay quiet.
  console.debug("Connecting to native host", HOST_NAME, "from extension", browser.runtime.id);

  let thisPort: Browser.runtime.Port;
  try {
    thisPort = browser.runtime.connectNative(HOST_NAME);
  } catch (e) {
    console.warn("connectNative threw:", e instanceof Error ? e.message : String(e));
    setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
    emitStatus({
      type: "typeError",
      key: "connected",
      error: "could not connect to native host",
    });
    return;
  }

  port = thisPort;
  thisPort.onDisconnect.addListener(() => onDisconnect(thisPort));
  thisPort.onMessage.addListener((raw) => onPortMessage(raw, thisPort));

  thisPort.postMessage({ type: "typeVersion" });
  thisPort.postMessage({ type: "typeDevices" });
}

/** Handle a port disconnection: drop the port, report the error, retry with backoff. */
function onDisconnect(thisPort: Browser.runtime.Port): void {
  if (thisPort !== port) return;
  port = null;

  // `lastError` is only valid inside the disconnect callback: read it now.
  const err = (thisPort as FirefoxPort).error ?? browser.runtime.lastError;

  clearDevices();
  void browser.contextMenus.removeAll();
  setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
  emitStatus({
    type: "typeError",
    key: "connected",
    error: err?.message ?? "native host disconnected",
  });

  const delay = nextRetryDelay(retry, Date.now());
  if (delay !== null) {
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, delay);
  }
}

/** Connect on demand; user-initiated calls get a fresh backoff budget. */
export function ensureConnection(): void {
  if (!getOptions().enabled) return;
  if (!port) connect(true);
}

/** Tear down the native port without scheduling a reconnect. */
export function disableIntegration(): void {
  const current = port;
  port = null;
  current?.disconnect();
  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  clearDevices();
  void browser.contextMenus.removeAll();
  clearBadge("connected");
}

/** Ensure a connection and post a `typeShare` to the host. */
export function postShare(data: ShareRequest): void {
  ensureConnection();
  if (!port) return;
  resetRetry(retry);
  port.postMessage(shareRequest(data.target, data.url, data.imageData));
}

/** Route a native-host message to the browser-side effects it produces. */
function onPortMessage(raw: unknown, thisPort: Browser.runtime.Port): void {
  if (thisPort !== port) return;

  const msg = raw as Message;

  if (msg.type === "typeVersion" || msg.type === "typeDevices") {
    markHandshake(retry, Date.now());
    clearBadge("connected");
    emitClearStatus("connected");
  }

  for (const effect of translateHostMessage(msg, PROTOCOL_VERSION)) {
    applyEffect(effect);
  }
}

/** Apply one translated host effect to the device cache, menus, and status. */
function applyEffect(effect: HostEffect): void {
  switch (effect.kind) {
    case "devices":
      setDevices(effect.devices);
      rebuildContextMenus();
      emitDevices(effect.devices);
      break;

    case "device":
      updateDevice(effect.device);
      rebuildContextMenus();
      break;

    case "error":
      emitStatus({ type: "typeError", key: effect.key, error: effect.error });
      break;

    case "shareResult":
      if (effect.ok) {
        emitStatus({ type: "typeShareResult", key: "share", message: "Sent" });
      } else {
        emitStatus({
          type: "typeError",
          key: "share",
          error: effect.error ?? "share failed",
        });
      }
      break;

    case "version":
      if (!effect.matches) {
        setBadge("update", { text: "!", color: [0, 116, 255, 220] });
        emitStatus({
          type: "typeVersion",
          key: "update",
          update: PROTOCOL_VERSION,
          current: effect.remote,
        });
      } else {
        clearBadge("update");
        emitClearStatus("update");
      }
      break;
  }
}

/** Dispatch a `typeShare` from a menu item click to the native host. */
export async function handleMenuClick(
  info: Browser.contextMenus.OnClickData,
  tab?: Browser.tabs.Tab,
): Promise<void> {
  const parsed = parseMenuId(String(info.menuItemId));
  if (!parsed) return;

  const url = resolveMenuUrl(parsed.action, info, tab?.url);
  if (!url) return;

  // Image context: transfer the file data rather than the link.
  const imageData = wantsImageData(info) ? await shareImageData(info) : null;

  resetRetry(retry);
  ensureConnection();
  port?.postMessage(shareRequest(parsed.deviceId, url, imageData));
}

/** Fetch the image bytes of an image-context click, or null if unavailable. */
async function shareImageData(info: Browser.contextMenus.OnClickData): Promise<string | null> {
  const url = info.srcUrl ?? info.linkUrl;
  if (!url) return null;
  try {
    const res = await fetch(url);
    if (!res.ok) return null;

    // Reject before downloading when the server advertises an oversized body.
    const declared = Number(res.headers.get("content-length"));
    if (Number.isFinite(declared) && declared > MAX_IMAGE_BYTES) return null;

    const blob = await res.blob();
    if (!blob.type.startsWith("image/") || !imageBytesAllowed(blob.size)) return null;
    return bytesToBase64(new Uint8Array(await blob.arrayBuffer()));
  } catch {
    return null; // fall back to sharing the URL
  }
}

function bytesToBase64(bytes: Uint8Array): string {
  let out = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    out += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(out);
}
