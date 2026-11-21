/**
 * Options page — entry point: saving, restoring, messaging.
 */

import type { Message } from "../generated/types";
import { applyMessage } from "../device-list";
import { getStoredOptions } from "../options";
import { setDefaultDeviceId, getDefaultDeviceId } from "./state";
import { writeDevices, setSaveStatus, toggleDefaultDevice } from "./render";

function sendMessage(msg: Message): void {
  chrome.runtime.sendMessage(msg).catch(() => {});
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
      setDefaultDeviceId(newDefaultDeviceId);
      setSaveStatus(chrome.runtime.lastError ?? null);
    },
  );
}

function restoreOptions(): void {
  getStoredOptions((items) => {
    setDefaultDeviceId(items.defaultDeviceId);
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
      defaultDevice.value = getDefaultDeviceId() ?? "";
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
