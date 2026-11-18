/**
 * Background service worker (MV3).
 *
 * Owns the native messaging port to `com.0xc0dedbad.kdeconnect_chrome`,
 * caches device state, updates context menus and badge, and routes
 * messages between the native host and popup/options views.
 */

import type { Device, Message, ExtensionOptions } from "./generated/types.js";

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

function connect(): void {
  clearBadge("connected");
  broadcast({ type: "typeClearStatus", data: { key: "connected" } });

  // Debug: log extension ID and hostname for troubleshooting
  const extId = chrome.runtime.id;
  console.info("Connecting to native host", HOSTNAME, "from extension", extId);

  try {
    port = chrome.runtime.connectNative(HOSTNAME);
  } catch (e) {
    console.warn("connectNative threw:", e instanceof Error ? e.message : String(e));
    setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
    broadcast({
      type: "typeStatus",
      data: { type: "typeError", key: "connected", error: "could not connect to native host" },
    });
    return;
  }

  port.onDisconnect.addListener(onDisconnect);
  port.onMessage.addListener(onPortMessage);

  port.postMessage({ type: "typeVersion" });
  port.postMessage({ type: "typeDevices" });
}

function onDisconnect(): void {
  port = null;
  setBadge("connected", { text: "!", color: [255, 0, 0, 220] });
  broadcast({
    type: "typeStatus",
    data: { type: "typeError", key: "connected", error: "could not connect to native host" },
  });

  const err = chrome.runtime.lastError;
  console.warn(
    "Disconnected from native host",
    JSON.stringify({
      message: err?.message,
      hasError: !!err,
      keys: err ? Object.keys(err) : [],
    }),
  );
}

/**
 * Route messages from popup/options to the native host.
 *
 * The native port lives in the background, so view requests (`typeDevices`,
 * `typeVersion`) and share actions must be forwarded. Host replies are pushed
 * back to every view by `onPortMessage` → `broadcast`.
 */
function onViewMessage(
  msg: unknown,
  _sender: chrome.runtime.MessageSender,
  _sendResponse: (response?: unknown) => void,
): void {
  if (!port) connect();
  port?.postMessage(msg);
}

// Must be synchronous; we drop the result.
chrome.runtime.onMessage.addListener(onViewMessage);

// ── Native message handling ───────────────────────────────────────────

function onPortMessage(raw: unknown): void {
  const msg = raw as Message;
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

function rebuildContextMenus(): void {
  if (options.disableContextMenu) {
    void chrome.contextMenus.removeAll();
    return;
  }

  chrome.contextMenus.removeAll(() => {
    const pool =
      options.defaultOnly && options.defaultDeviceId
        ? { [options.defaultDeviceId]: knownDevices[options.defaultDeviceId] }
        : knownDevices;

    const entries = Object.values(pool).filter(Boolean);
    const hasActive = entries.some((d) => d.isReachable && d.isTrusted);

    if (!hasActive) {
      setBadge("active", { text: "!", color: [255, 129, 0, 220] });
      return;
    }

    if (entries.length === 0) return;
    clearBadge("active");

    if (entries.length === 1) {
      const d = entries[0];
      chrome.contextMenus.create({
        id: d.id,
        title: `KDE Connect (${d.name})`,
        enabled: d.isReachable && d.isTrusted,
        contexts: ["page", "frame", "link", "image", "video", "audio"],
      });
      return;
    }

    chrome.contextMenus.create({
      id: "kdeconnectRoot",
      title: "KDE Connect",
      contexts: ["page", "frame", "link", "image", "video", "audio"],
    });

    for (const d of entries) {
      chrome.contextMenus.create({
        id: d.id,
        title: d.name,
        enabled: d.isReachable && d.isTrusted,
        parentId: "kdeconnectRoot",
        contexts: ["page", "frame", "link", "image", "video", "audio"],
      });
    }
  });
}

chrome.contextMenus.onClicked.addListener(
  (info: chrome.contextMenus.OnClickData, tab?: chrome.tabs.Tab) => {
    if (!port || !info.menuItemId) return;
    if (info.menuItemId === "kdeconnectRoot") return;

    port.postMessage({
      type: "typeShare",
      data: {
        target: info.menuItemId,
        url: info.linkUrl || info.srcUrl || info.frameUrl || info.pageUrl || tab?.url,
      },
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
