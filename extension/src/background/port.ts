/**
 * Native-messaging port lifecycle and request routing.
 */

import type { Device, Message } from "../generated/types";
import {
  broadcast,
  clearBadge,
  clearDevices,
  getOptions,
  setBadge,
  setDevices,
  updateDevice,
} from "./state";
import { rebuildContextMenus } from "./menus";
import { parseMenuId, resolveMenuUrl } from "../menus";
import { createRetryState, markHandshake, nextRetryDelay, resetRetry } from "../reconnect";
import type { RetryState } from "../reconnect";
import { shareRequest, wantsImageData, imageBytesAllowed, MAX_IMAGE_BYTES } from "../share";

const HOSTNAME = "com.0xc0dedbad.kdeconnect_chrome";
const PROTOCOL_VERSION = "0.1.4";

let port: chrome.runtime.Port | null = null;
let retry: RetryState = createRetryState();
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

/** Firefox sets `port.error` on disconnect instead of `lastError`. */
interface FirefoxPort {
  error?: { message?: string };
}

export function connect(isUserAction = false): void {
  if (port || !getOptions().enabled) return;

  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  retry.handshakeAt = null;
  if (isUserAction) resetRetry(retry);

  clearBadge("connected");
  broadcast({ type: "typeClearStatus", data: { key: "connected" } });

  // Debug: log extension ID and hostname for troubleshooting
  const extId = chrome.runtime.id;
  console.info("Connecting to native host", HOSTNAME, "from extension", extId);

  let thisPort: chrome.runtime.Port;
  try {
    thisPort = chrome.runtime.connectNative(HOSTNAME);
  } catch (e) {
    console.warn("connectNative threw:", e instanceof Error ? e.message : String(e));
    setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
    broadcast({
      type: "typeStatus",
      data: { type: "typeError", key: "connected", error: "could not connect to native host" },
    });
    return;
  }

  port = thisPort;
  thisPort.onDisconnect.addListener(() => onDisconnect(thisPort));
  thisPort.onMessage.addListener((raw) => onPortMessage(raw, thisPort));

  thisPort.postMessage({ type: "typeVersion" });
  thisPort.postMessage({ type: "typeDevices" });
}

function onDisconnect(thisPort: chrome.runtime.Port): void {
  if (thisPort !== port) return;
  port = null;

  // `lastError` is only valid inside the disconnect callback: read it now.
  const err = (thisPort as FirefoxPort).error ?? chrome.runtime.lastError;

  clearDevices();
  void chrome.contextMenus.removeAll();
  setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
  broadcast({
    type: "typeStatus",
    data: {
      type: "typeError",
      key: "connected",
      error: err?.message ?? "native host disconnected",
    },
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
function ensurePort(): chrome.runtime.Port | null {
  if (!getOptions().enabled) return null;
  if (!port) connect(true);
  return port;
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
  void chrome.contextMenus.removeAll();
  clearBadge("connected");
}

/**
 * Route messages from popup/options to the native host.
 *
 * The native port lives in the background, so view requests (`typeDevices`,
 * `typeVersion`) and share actions must be forwarded. Host replies are pushed
 * back to every view by `onPortMessage` → `broadcast`.
 */
function onViewMessage(msg: unknown): void {
  const type = (msg as { type?: unknown } | null)?.type;
  if (type !== "typeVersion" && type !== "typeDevices" && type !== "typeShare") {
    return;
  }
  ensurePort()?.postMessage(msg);
}

// Must be synchronous; we drop the result.
chrome.runtime.onMessage.addListener(onViewMessage);

function onPortMessage(raw: unknown, thisPort: chrome.runtime.Port): void {
  if (thisPort !== port) return;

  const msg = raw as Message;

  if (msg.type === "typeVersion" || msg.type === "typeDevices") {
    markHandshake(retry, Date.now());
    clearBadge("connected");
    broadcast({ type: "typeClearStatus", data: { key: "connected" } });
  }

  switch (msg.type) {
    case "typeDeviceUpdate":
      updateDevice(msg.data as Device);
      rebuildContextMenus();
      broadcast(msg);
      break;

    case "typeDevices": {
      setDevices(msg.data as Record<string, Device>);
      rebuildContextMenus();
      broadcast(msg);
      break;
    }

    case "typeError":
      broadcast({
        type: "typeStatus",
        data: { type: "typeError", key: "host", error: msg.data as string },
      });
      break;

    case "typeShareResult": {
      const data = msg.data as { ok?: boolean; error?: string } | undefined;
      broadcast(
        data?.ok
          ? {
              type: "typeStatus",
              data: { type: "typeShareResult", key: "share", message: "Sent" },
            }
          : {
              type: "typeStatus",
              data: {
                type: "typeError",
                key: "share",
                error: String(data?.error ?? "share failed"),
              },
            },
      );
      break;
    }

    case "typeVersion": {
      const remote = msg.data as string;
      if (remote !== PROTOCOL_VERSION) {
        setBadge("update", { text: "!", color: [0, 116, 255, 220] });
        broadcast({
          type: "typeStatus",
          data: { type: "typeVersion", key: "update", update: PROTOCOL_VERSION, current: remote },
        });
      } else {
        clearBadge("update");
        broadcast({ type: "typeClearStatus", data: { key: "update" } });
      }
      break;
    }

    default:
      broadcast(msg);
  }
}

chrome.contextMenus.onClicked.addListener(handleMenuClick);

/** Dispatch a `typeShare` from a menu item click to the native host. */
async function handleMenuClick(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab,
): Promise<void> {
  const parsed = parseMenuId(String(info.menuItemId));
  if (!parsed) return;

  const url = resolveMenuUrl(parsed.action, info, tab?.url);
  if (!url) return;

  // Image context: transfer the file data rather than the link.
  const imageData = wantsImageData(info) ? await shareImageData(info) : null;

  resetRetry(retry);
  ensurePort()?.postMessage(shareRequest(parsed.deviceId, url, imageData));
}

/** Fetch the image bytes of an image-context click, or null if unavailable. */
async function shareImageData(info: chrome.contextMenus.OnClickData): Promise<string | null> {
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
