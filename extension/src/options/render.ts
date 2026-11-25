/**
 * Options page — device-select rendering and save status.
 */

import { Check, createIcons } from "lucide";

import { renderDeviceList } from "../device-list";
import type { Device } from "../generated/types";
import { getDefaultDeviceId, clearSaveStatusTimer, setSaveStatusTimer } from "./state";

/** Fill the `defaultDevice` select with one option per device. */
export function writeDevices(devices: Record<string, Device>): void {
  const select = document.getElementById("defaultDevice") as HTMLSelectElement | null;
  if (!select) return;

  const selectedDeviceId = select.value || getDefaultDeviceId();
  renderDeviceList(select, devices, (device) => {
    const opt = document.createElement("option");
    opt.value = device.id;
    opt.disabled = !(device.isReachable && device.isTrusted);
    opt.textContent = device.name;
    return opt;
  });

  select.value = selectedDeviceId ?? "";
}

/** Toggle the visibility of the default-device container. */
export function toggleDefaultDevice(show: boolean): void {
  const el = document.getElementById("defaultDeviceContainer");
  if (el) el.classList.toggle("hidden", !show);
}

/** Show a save status line that clears after a moment. */
export function setSaveStatus(error: { message?: string } | null): void {
  const status = document.getElementById("status");
  if (!status) return;

  clearSaveStatusTimer();

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

  setSaveStatusTimer(
    setTimeout(() => {
      status.innerHTML = "";
      setSaveStatusTimer(null);
    }, 750),
  );
}
