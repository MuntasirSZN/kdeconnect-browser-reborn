/**
 * Context-menu rebuilding.
 */

import { browser } from "wxt/browser";

import { getDevices } from "../device-cache";
import { getOptions } from "../settings";
import { buildMenuSpec } from "./spec";
import type { MenuStrings } from "./spec";

const IS_FIREFOX = /firefox/i.test(navigator.userAgent);

let menuRebuildInProgress = false;
let menuRebuildQueued = false;

function message(key: string, subs: string[] = [], fallback = ""): string {
  return (
    browser.i18n.getMessage(key as Parameters<typeof browser.i18n.getMessage>[0], subs) || fallback
  );
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
    browser.contextMenus.removeAll(() => {
      // Coalesce updates that arrived while removeAll was pending. Running
      // overlapping removeAll callbacks can otherwise leave stale/empty menus.
      if (menuRebuildQueued) {
        rebuild();
        return;
      }

      for (const entry of buildMenuSpec(getDevices(), getOptions(), menuStrings, IS_FIREFOX)) {
        void browser.contextMenus.create(entry, () => void browser.runtime.lastError);
      }

      menuRebuildInProgress = false;
      if (menuRebuildQueued) rebuildContextMenus();
    });
  };

  rebuild();
}
