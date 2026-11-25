/**
 * Options page — entry point: saving, restoring, messaging.
 */

import { onMessage, sendMessage } from "../../messaging";
import { loadOptions, saveOptions } from "../../storage";
import type { ExtensionOptions } from "../../shared-types";
import { setDefaultDeviceId, getDefaultDeviceId } from "../../options/state";
import { writeDevices, setSaveStatus, toggleDefaultDevice } from "../../options/render";

/** Read the options form into the `ExtensionOptions` shape. */
function readOptions(): ExtensionOptions {
  return {
    enabled: (document.getElementById("enabled") as HTMLInputElement)?.checked ?? true,
    defaultOnly: (document.getElementById("defaultOnly") as HTMLInputElement)?.checked ?? false,
    defaultDeviceId: (document.getElementById("defaultDevice") as HTMLSelectElement)?.value || null,
    disableContextMenu:
      (document.getElementById("disableContextMenu") as HTMLInputElement)?.checked ?? false,
  };
}

/** Persist `readOptions()` and clear the save status on success. */
async function save(): Promise<void> {
  try {
    await saveOptions(readOptions());
    setSaveStatus(null);
  } catch (e: unknown) {
    setSaveStatus({ message: e instanceof Error ? e.message : String(e) });
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  const opts = await loadOptions();
  setDefaultDeviceId(opts.defaultDeviceId);

  const enabled = document.getElementById("enabled") as HTMLInputElement | null;
  const defaultOnly = document.getElementById("defaultOnly") as HTMLInputElement | null;
  const disableContextMenu = document.getElementById(
    "disableContextMenu",
  ) as HTMLInputElement | null;
  const defaultDevice = document.getElementById("defaultDevice") as HTMLSelectElement | null;

  if (enabled) enabled.checked = opts.enabled;
  if (defaultOnly) {
    defaultOnly.checked = opts.defaultOnly;
    defaultOnly.addEventListener("change", (e) =>
      toggleDefaultDevice((e.target as HTMLInputElement).checked),
    );
  }
  if (disableContextMenu) disableContextMenu.checked = opts.disableContextMenu;
  if (defaultDevice) defaultDevice.value = getDefaultDeviceId() ?? "";

  toggleDefaultDevice(opts.defaultOnly);
  writeDevices(await sendMessage("getDevices"));

  document.getElementById("save")?.addEventListener("click", () => void save());
});

onMessage("devicesChanged", ({ data }) => writeDevices(data));
