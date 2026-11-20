/**
 * Options page — default device, context menu toggle.
 */

import type { Device, Message } from "../generated/types.js";

// ── State ──────────────────────────────────────────────────────────────

let defaultDeviceId: string | null = null;

// ── Messaging ──────────────────────────────────────────────────────────

function sendMessage(msg: Message): void {
  chrome.runtime.sendMessage(msg).catch(() => {});
}

// ── Device list ────────────────────────────────────────────────────────

function writeDevices(devices: Record<string, Device>): void {
  const select = document.getElementById("defaultDevice") as HTMLSelectElement | null;
  if (!select) return;

  select.innerHTML = "";

  for (const dev of Object.values(devices).filter(Boolean)) {
    const opt = document.createElement("option");
    opt.value = dev.id;
    opt.disabled = !(dev.isReachable && dev.isTrusted);
    opt.selected = defaultDeviceId === dev.id;
    opt.textContent = dev.name;
    select.appendChild(opt);
  }
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

      const error = chrome.runtime.lastError;
      if (error) {
        status.textContent = `Save failed: ${error.message ?? "unknown error"}`;
        return;
      }

      status.textContent = "Saved…";
      setTimeout(() => {
        status.textContent = "";
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
    writeDevices((msg.data ?? {}) as Record<string, Device>);
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
