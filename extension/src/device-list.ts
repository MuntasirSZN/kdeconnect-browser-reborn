import type { Device } from "./generated/types";

/**
 * Clear `container` and append one element per device via `render`.
 * Null renders are skipped.
 */
export function renderDeviceList(
  container: HTMLElement,
  devices: Record<string, Device>,
  render: (device: Device) => HTMLElement | null,
): void {
  container.innerHTML = "";
  for (const device of Object.values(devices).filter(Boolean)) {
    const el = render(device);
    if (el) container.appendChild(el);
  }
}
