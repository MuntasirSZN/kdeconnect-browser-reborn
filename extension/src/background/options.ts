/**
 * Options application and the `chrome.storage.onChanged` listener.
 */

import { connect, disableIntegration } from "./port";
import { rebuildContextMenus } from "./menus";
import { getOptions, setOption, setOptions } from "./state";
import { getStoredOptions } from "../options";

/** Load persisted options; connect if enabled, clear menus otherwise. */
export function loadOptions(): void {
  getStoredOptions((items) => {
    setOptions(items);
    if (getOptions().enabled) connect();
    else disableIntegration();
    rebuildContextMenus();
  });
}

chrome.storage.onChanged.addListener((changes: Record<string, chrome.storage.StorageChange>) => {
  let changed = false;

  if (changes.enabled !== undefined) {
    setOption("enabled", changes.enabled.newValue as boolean);
    if (!getOptions().enabled) disableIntegration();
    else connect(true);
    changed = true;
  }
  if (changes.defaultDeviceId !== undefined) {
    setOption("defaultDeviceId", changes.defaultDeviceId.newValue as string | null);
    changed = true;
  }
  if (changes.defaultOnly !== undefined) {
    setOption("defaultOnly", changes.defaultOnly.newValue as boolean);
    changed = true;
  }
  if (changes.disableContextMenu !== undefined) {
    setOption("disableContextMenu", changes.disableContextMenu.newValue as boolean);
    changed = true;
  }

  if (changed) rebuildContextMenus();
});
