/**
 * Popup view — lists KDE Connect devices with send button.
 */

import { createIcons } from "lucide";
import type { Device, Message, StatusData } from "../generated/types.js";
import { DEVICE_ICONS, typeIcon } from "../icons.js";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let knownDevices: Record<string, Device> = {};
let currentUrlPromise: Promise<string | null> | null = null;

// ── Messaging ──────────────────────────────────────────────────────────

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

// ── Device list rendering ──────────────────────────────────────────────

function renderDevice(device: Device): HTMLElement | null {
  if (!device) return null;

  const disabled = !(device.isReachable && device.isTrusted);
  const el = document.createElement("button");
  el.className = "device";
  el.dataset.id = device.id;
  el.disabled = disabled;

  const icon = document.createElement("i");
  icon.dataset.lucide = typeIcon(device.type);
  icon.className = "device-icon";
  icon.setAttribute("aria-hidden", "true");

  const name = document.createElement("span");
  name.className = "device-name";
  name.textContent = device.name;

  const status = document.createElement("span");
  status.className = `device-status ${device.isReachable ? "online" : "offline"}`;
  status.setAttribute("aria-hidden", "true");

  const send = document.createElement("span");
  send.className = "device-send";
  send.textContent = "Send";

  el.append(icon, name, status, send);

  el.addEventListener("click", () => {
    void getCurrentTabUrl().then((url) => {
      if (url) {
        postMessage({ type: "typeShare", data: { target: device.id, url } });
      }
      window.close();
    });
  });

  return el;
}

function writeDevices(devices: Record<string, Device>): void {
  const container = document.getElementById("devices");
  if (!container) return;

  container.innerHTML = "";

  const entries = Object.values(devices).filter(Boolean);
  if (entries.length === 0) {
    container.innerHTML = '<p class="empty">No devices found…</p>';
    return;
  }

  for (const dev of entries) {
    const el = renderDevice(dev);
    if (el) container.appendChild(el);
  }

  createIcons({ icons: DEVICE_ICONS });
}

// ── Status messages ────────────────────────────────────────────────────

function addStatus(msg: Message): void {
  const data = msg.data as StatusData | undefined;
  if (!data?.key) return;

  const container = document.getElementById("status");
  if (!container) return;

  let el = document.getElementById(`status-${data.key}`);
  if (!el) {
    el = document.createElement("p");
    el.id = `status-${data.key}`;
    el.className = "status";
    container.appendChild(el);
  }

  el.innerHTML = "";

  switch (data.type) {
    case "typeVersion": {
      el.innerHTML = `Host upgrade v${String(data.update)} available (current: v${String(data.current)}). `;
      const link = document.createElement("a");
      link.href = "https://github.com/pdf/kdeconnect-chrome-extension#upgrading";
      link.target = "_blank";
      link.textContent = "Upgrade instructions";
      el.appendChild(link);
      break;
    }
    case "typeError":
      el.className = "status error";
      el.textContent = `Error: ${String(data.error ?? "unknown")}`;
      break;
    default:
      el.textContent = String(data.error ?? "");
  }
}

function removeStatus(msg: Message): void {
  const data = msg.data as StatusData | undefined;
  const key = data?.key;
  if (!key) return;
  const el = document.getElementById(`status-${key}`);
  el?.remove();
}

// ── Message handler (from background) ──────────────────────────────────

function onMessage(msg: Record<string, unknown>): void {
  switch (msg.type as string) {
    case "typeDeviceUpdate":
      updateDevice(msg.data as Device);
      break;
    case "typeDevices": {
      knownDevices = (msg.data ?? {}) as Record<string, Device>;
      writeDevices(knownDevices);
      break;
    }
    case "typeStatus":
      addStatus(msg as Message);
      break;
    case "typeClearStatus":
      removeStatus(msg as Message);
      break;
  }
}

function updateDevice(device: Device): void {
  knownDevices[device.id] = device;
  writeDevices(knownDevices);
}

// ── Init ───────────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", () => {
  init().catch((e: unknown) => {
    console.warn("popup init failed", e);
  });
});

async function init(): Promise<void> {
  const items = (await chrome.storage.sync.get({
    defaultOnly: false,
    defaultDeviceId: null,
  })) as { defaultOnly?: boolean; defaultDeviceId?: string | null };

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

chrome.runtime.onMessage.addListener((msg: unknown) => {
  onMessage(msg as Message);
});
