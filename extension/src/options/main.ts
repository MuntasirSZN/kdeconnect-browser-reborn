/**
 * Options page — default device, context menu toggle.
 */

import type { Device, Message } from "../generated/types.js";

// ── State ──────────────────────────────────────────────────────────────

let defaultDeviceId: string | null = null;
let knownDevices: Record<string, Device> = {};
let saveStatusTimeout: ReturnType<typeof setTimeout> | null = null;

// ── Messaging ──────────────────────────────────────────────────────────

function sendMessage(msg: Message): void {
  chrome.runtime.sendMessage(msg).catch(() => {});
}

// ── Device list ────────────────────────────────────────────────────────

function writeDevices(devices: Record<string, Device>): void {
  const select = document.getElementById("defaultDevice") as HTMLSelectElement | null;
  if (!select) return;

  const selectedDeviceId = select.value || defaultDeviceId;
  select.innerHTML = "";

  for (const dev of Object.values(devices).filter(Boolean)) {
    const opt = document.createElement("option");
    opt.value = dev.id;
    opt.disabled = !(dev.isReachable && dev.isTrusted);
    opt.textContent = dev.name;
    select.appendChild(opt);
  }

  select.value = selectedDeviceId ?? "";
}

// ── Toggle helpers ─────────────────────────────────────────────────────

function toggleDefaultDevice(show: boolean): void {
  const el = document.getElementById("defaultDeviceContainer");
  if (el) el.classList.toggle("hidden", !show);
}

// ── Save / restore ─────────────────────────────────────────────────────

function saveOptions(): void {
  const newDefaultDeviceId =
    (document.getElementById("defaultDevice") as HTMLSelectElement)?.value ?? "";
  const newDefaultOnly =
    (document.getElementById("defaultOnly") as HTMLInputElement)?.checked ?? false;
  const newDisableContextMenu =
    (document.getElementById("disableContextMenu") as HTMLInputElement)?.checked ?? false;

  chrome.storage.sync.set(
    {
      defaultDeviceId: newDefaultDeviceId,
      defaultOnly: newDefaultOnly,
      disableContextMenu: newDisableContextMenu,
    },
    () => {
      const status = document.getElementById("status");
      if (!status) return;

      if (saveStatusTimeout !== null) {
        clearTimeout(saveStatusTimeout);
        saveStatusTimeout = null;
      }

      const error = chrome.runtime.lastError;
      if (error) {
        status.textContent = `Save failed: ${error.message ?? "unknown error"}`;
        return;
      }

      defaultDeviceId = newDefaultDeviceId;
      status.textContent = "Saved…";
      saveStatusTimeout = setTimeout(() => {
        status.textContent = "";
        saveStatusTimeout = null;
      }, 750);
    },
  );
}

function restoreOptions(): void {
  chrome.storage.sync.get(
    { defaultOnly: false, defaultDeviceId: null, disableContextMenu: false },
    (items) => {
      defaultDeviceId = items.defaultDeviceId as string | null;
      const defaultOnly = document.getElementById("defaultOnly") as HTMLInputElement;
      const disableContextMenu = document.getElementById("disableContextMenu") as HTMLInputElement;
      const defaultDevice = document.getElementById("defaultDevice") as HTMLSelectElement;

      if (defaultOnly) {
        defaultOnly.checked = items.defaultOnly as boolean;
        toggleDefaultDevice(items.defaultOnly as boolean);
        defaultOnly.addEventListener("change", (e) => {
          toggleDefaultDevice((e.target as HTMLInputElement).checked);
        });
      }
      if (disableContextMenu) {
        disableContextMenu.checked = items.disableContextMenu as boolean;
      }
      if (defaultDevice) {
        defaultDevice.value = defaultDeviceId ?? "";
      }
    },
  );
}

// ── Message handler ────────────────────────────────────────────────────

function onMessage(msg: Message): void {
  if (msg.type === "typeDevices") {
    knownDevices = (msg.data ?? {}) as Record<string, Device>;
    writeDevices(knownDevices);
    return;
  }

  if (msg.type === "typeDeviceUpdate") {
    const device = msg.data as Device | undefined;
    if (!device?.id) return;
    knownDevices[device.id] = device;
    writeDevices(knownDevices);
  }
}

// ── Init ───────────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", () => {
  restoreOptions();
  sendMessage({ type: "typeDevices" });

  document.getElementById("save")?.addEventListener("click", saveOptions);
});

chrome.runtime.onMessage.addListener((msg: unknown) => {
  onMessage(msg as Message);
});
