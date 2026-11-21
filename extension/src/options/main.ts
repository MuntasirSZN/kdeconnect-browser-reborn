/**
 * Options page — default device, context menu toggle.
 */

import { Check, createIcons } from "lucide";
import type { Message } from "../generated/types";
import { applyMessage, knownDevices, renderDeviceList } from "../device-list";
import { getStoredOptions } from "../options";

let defaultDeviceId: string | null = null;
let saveStatusTimeout: ReturnType<typeof setTimeout> | null = null;

function sendMessage(msg: Message): void {
  chrome.runtime.sendMessage(msg).catch(() => {});
}

function writeDevices(): void {
  const select = document.getElementById("defaultDevice") as HTMLSelectElement | null;
  if (!select) return;

  const selectedDeviceId = select.value || defaultDeviceId;
  renderDeviceList(select, knownDevices, (device) => {
    const opt = document.createElement("option");
    opt.value = device.id;
    opt.disabled = !(device.isReachable && device.isTrusted);
    opt.textContent = device.name;
    return opt;
  });

  select.value = selectedDeviceId ?? "";
}

function toggleDefaultDevice(show: boolean): void {
  const el = document.getElementById("defaultDeviceContainer");
  if (el) el.classList.toggle("hidden", !show);
}

function setSaveStatus(error: { message?: string } | null): void {
  const status = document.getElementById("status");
  if (!status) return;

  if (saveStatusTimeout !== null) {
    clearTimeout(saveStatusTimeout);
    saveStatusTimeout = null;
  }

  status.innerHTML = "";

  if (error) {
    status.classList.add("error");
    status.textContent = `Save failed: ${error.message ?? "unknown error"}`;
    return;
  }

  status.classList.remove("error");
  const icon = document.createElement("i");
  icon.dataset.lucide = "Check";
  icon.setAttribute("aria-hidden", "true");
  icon.className = "status-icon";
  status.append(icon, document.createTextNode("Saved…"));
  createIcons({ icons: { Check } });

  saveStatusTimeout = setTimeout(() => {
    status.innerHTML = "";
    saveStatusTimeout = null;
  }, 750);
}

function saveOptions(): void {
  const newEnabled = (document.getElementById("enabled") as HTMLInputElement)?.checked ?? true;
  const newDefaultDeviceId =
    (document.getElementById("defaultDevice") as HTMLSelectElement)?.value ?? "";
  const newDefaultOnly =
    (document.getElementById("defaultOnly") as HTMLInputElement)?.checked ?? false;
  const newDisableContextMenu =
    (document.getElementById("disableContextMenu") as HTMLInputElement)?.checked ?? false;

  chrome.storage.sync.set(
    {
      enabled: newEnabled,
      defaultDeviceId: newDefaultDeviceId,
      defaultOnly: newDefaultOnly,
      disableContextMenu: newDisableContextMenu,
    },
    () => {
      defaultDeviceId = newDefaultDeviceId;
      setSaveStatus(chrome.runtime.lastError ?? null);
    },
  );
}

function restoreOptions(): void {
  getStoredOptions((items) => {
    defaultDeviceId = items.defaultDeviceId;
    const enabled = document.getElementById("enabled") as HTMLInputElement;
    const defaultOnly = document.getElementById("defaultOnly") as HTMLInputElement;
    const disableContextMenu = document.getElementById("disableContextMenu") as HTMLInputElement;
    const defaultDevice = document.getElementById("defaultDevice") as HTMLSelectElement;

    if (enabled) {
      enabled.checked = items.enabled;
    }
    if (defaultOnly) {
      defaultOnly.checked = items.defaultOnly;
      toggleDefaultDevice(items.defaultOnly);
      defaultOnly.addEventListener("change", (e) => {
        toggleDefaultDevice((e.target as HTMLInputElement).checked);
      });
    }
    if (disableContextMenu) {
      disableContextMenu.checked = items.disableContextMenu;
    }
    if (defaultDevice) {
      defaultDevice.value = defaultDeviceId ?? "";
    }
  });
}

function onMessage(msg: Message): void {
  applyMessage(msg, writeDevices);
}

document.addEventListener("DOMContentLoaded", () => {
  restoreOptions();
  sendMessage({ type: "typeDevices" });

  document.getElementById("save")?.addEventListener("click", saveOptions);
});

chrome.runtime.onMessage.addListener((msg: unknown) => {
  onMessage(msg as Message);
});
