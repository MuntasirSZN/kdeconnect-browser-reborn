/**
 * Popup view — device-list rendering.
 */

import { createIcons } from "lucide";
import type { Device } from "../generated/types";
import { DEVICE_ICONS, typeIcon } from "../icons";
import { renderDeviceList } from "../device-list";

/** Render the empty-devices placeholder. */
export function renderEmpty(text: string): void {
  const container = document.getElementById("devices");
  if (!container) return;

  container.innerHTML = "";

  const el = document.createElement("p");
  el.className = "empty";

  const icon = document.createElement("i");
  icon.dataset.lucide = "Smartphone";
  icon.setAttribute("aria-hidden", "true");
  el.append(icon, document.createTextNode(text));
  container.appendChild(el);

  createIcons({ icons: DEVICE_ICONS });
}

/** Build the device button; `onShare` sends it to the host. */
export function renderDevice(device: Device, onShare: () => void): HTMLElement | null {
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
    onShare();
  });

  return el;
}

/** Replace the device list with one button per device. */
export function writeDevices(
  devices: Record<string, Device>,
  onShare: (device: Device) => void,
): void {
  const container = document.getElementById("devices");
  if (!container) return;

  renderDeviceList(container, devices, (device) => renderDevice(device, () => onShare(device)));
  if (container.children.length === 0) {
    renderEmpty("No devices found…");
  } else {
    createIcons({ icons: DEVICE_ICONS });
  }
}
