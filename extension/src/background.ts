/**
 * Background service worker (MV3).
 *
 * Owns the native messaging port to `com.0xc0dedbad.kdeconnect_chrome`,
 * caches device state, updates context menus and badge, and routes
 * messages between the native host and popup/options views.
 */

import type { Device, Message, ExtensionOptions } from "./generated/types.js";
import { buildMenuSpec, parseMenuId, resolveMenuUrl } from "./menus.js";
import type { MenuStrings } from "./menus.js";
import { createRetryState, markHandshake, nextRetryDelay, resetRetry } from "./reconnect.js";
import type { RetryState } from "./reconnect.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const HOSTNAME = "com.0xc0dedbad.kdeconnect_chrome";
const PROTOCOL_VERSION = "0.1.3";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let port: chrome.runtime.Port | null = null;
let knownDevices: Record<string, Device> = {};
let options: ExtensionOptions = {
  defaultOnly: false,
  defaultDeviceId: null,
  disableContextMenu: false,
};

interface BadgeState {
  text: string;
  color: [number, number, number, number];
}

const badges: Record<string, BadgeState> = {};

/** Firefox sets `port.error` on disconnect instead of `lastError`. */
interface FirefoxPort {
  error?: { message?: string };
}

let retry: RetryState = createRetryState();
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

// ── Device cache (exported for popup/options) ──────────────────────────

/** Current device snapshot keyed by device ID. */
export function getDevices(): Readonly<Record<string, Device>> {
  return knownDevices;
}

/** Current user options. */
export function getOptions(): Readonly<ExtensionOptions> {
  return options;
}

// ── Native port ────────────────────────────────────────────────────────

function connect(isUserAction = false): void {
  if (port) return;

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

/** Drop cached devices and tell every view the list is empty. */
function clearDevices(): void {
  knownDevices = {};
  broadcast({ type: "typeDevices", data: {} });
}

/** Connect on demand; user-initiated calls get a fresh backoff budget. */
function ensurePort(): chrome.runtime.Port | null {
  if (!port) connect(true);
  return port;
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

// ── Native message handling ───────────────────────────────────────────

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
      broadcast(msg);
      break;

    case "typeDevices": {
      knownDevices = (msg.data ?? {}) as Record<string, Device>;
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

// ── Device cache ──────────────────────────────────────────────────────

function updateDevice(device: Device): void {
  knownDevices[device.id] = device;
  rebuildContextMenus();
}

// ── Badge ──────────────────────────────────────────────────────────────

function setBadge(source: string, state: BadgeState): void {
  badges[source] = state;
  applyBadge();
}

function clearBadge(source: string): void {
  delete badges[source];
  applyBadge();
}

function applyBadge(): void {
  const keys = Object.keys(badges);
  if (keys.length === 0) {
    void chrome.action.setBadgeText({ text: "" });
    return;
  }
  const last = badges[keys[keys.length - 1]];
  void chrome.action.setBadgeText({ text: last.text });
  void chrome.action.setBadgeBackgroundColor({ color: last.color });
}

// ── Context menus ─────────────────────────────────────────────────────

const message = (key: string, subs: string[] = [], fallback = ""): string =>
  chrome.i18n.getMessage(key, subs) || fallback;

const menuStrings: MenuStrings = {
  root: message("kdeconnect_root", [], "KDE Connect"),
  open: (name) => message("kdeconnect_open_device", [name], `Open on '${name}'`),
  call: (name) => message("kdeconnect_call_device", [name], `Call on '${name}'`),
};

function rebuildContextMenus(): void {
  chrome.contextMenus.removeAll(() => {
    for (const entry of buildMenuSpec(knownDevices, options, menuStrings)) {
      chrome.contextMenus.create(entry, () => void chrome.runtime.lastError);
    }
  });
}

chrome.contextMenus.onClicked.addListener(
  (info: chrome.contextMenus.OnClickData, tab?: chrome.tabs.Tab) => {
    const parsed = parseMenuId(String(info.menuItemId));
    if (!parsed) return;

    const url = resolveMenuUrl(parsed.action, info, tab?.url);
    if (!url) return;

    resetRetry(retry);
    ensurePort()?.postMessage({
      type: "typeShare",
      data: { target: parsed.deviceId, url },
    });
  },
);

// ── Message routing ───────────────────────────────────────────────────

/**
 * Broadcast a message to all extension views (popup, options).
 */
function broadcast(msg: unknown): void {
  chrome.runtime.sendMessage(msg).catch(() => {
    // Popup/options may be closed — ignore send errors.
  });
}

// ── Options (read from storage on start, listen for changes) ──────────

function loadOptions(): void {
  chrome.storage.sync.get(
    {
      defaultOnly: false,
      defaultDeviceId: null,
      disableContextMenu: false,
    } satisfies ExtensionOptions,
    (items: ExtensionOptions) => {
      options = items;
      rebuildContextMenus();
    },
  );
}

chrome.storage.onChanged.addListener((changes: Record<string, chrome.storage.StorageChange>) => {
  let changed = false;

  if (changes.defaultDeviceId !== undefined) {
    options.defaultDeviceId = changes.defaultDeviceId.newValue as string | null;
    changed = true;
  }
  if (changes.defaultOnly !== undefined) {
    options.defaultOnly = changes.defaultOnly.newValue as boolean;
    changed = true;
  }
  if (changes.disableContextMenu !== undefined) {
    options.disableContextMenu = changes.disableContextMenu.newValue as boolean;
    changed = true;
  }

  if (changed) rebuildContextMenus();
});

// ── Init ──────────────────────────────────────────────────────────────

loadOptions();
connect();
