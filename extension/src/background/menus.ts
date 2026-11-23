/**
 * Background context-menu rebuilding.
 */

import { getDevices, getOptions } from "./state";
import { buildMenuSpec } from "../menus/spec";
import type { MenuStrings } from "../menus/spec";

const IS_FIREFOX = /firefox/i.test(navigator.userAgent);

let menuRebuildInProgress = false;
let menuRebuildQueued = false;

function message(key: string, subs: string[] = [], fallback = ""): string {
  return chrome.i18n.getMessage(key, subs) || fallback;
}

const menuStrings: MenuStrings = {
  open: (name) => message("kdeconnect_open_device", [name], `Open on '${name}'`),
  call: (name) => message("kdeconnect_call_device", [name], `Call on '${name}'`),
};

export function rebuildContextMenus(): void {
  menuRebuildQueued = true;
  if (menuRebuildInProgress) return;

  menuRebuildInProgress = true;
  const rebuild = (): void => {
    menuRebuildQueued = false;
    chrome.contextMenus.removeAll(() => {
      // Coalesce updates that arrived while removeAll was pending. Running
      // overlapping removeAll callbacks can otherwise leave stale/empty menus.
      if (menuRebuildQueued) {
        rebuild();
        return;
      }

      for (const entry of buildMenuSpec(getDevices(), getOptions(), menuStrings, IS_FIREFOX)) {
        chrome.contextMenus.create(entry, () => void chrome.runtime.lastError);
      }

      menuRebuildInProgress = false;
      if (menuRebuildQueued) rebuildContextMenus();
    });
  };

  rebuild();
}
