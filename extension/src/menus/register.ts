/**
 * Context-menu rebuilding.
 */

import { browser } from "wxt/browser";

import { getDevices } from "../device-cache";
import { getOptions } from "../settings";
import { buildMenuSpec } from "./spec";
import type { MenuStrings } from "./spec";

import { i18n } from "#i18n";

const IS_FIREFOX = import.meta.env.BROWSER === "firefox";

let menuRebuildInProgress = false;
let menuRebuildQueued = false;

const menuStrings: MenuStrings = {
  open: (name) => i18n.t("kdeconnect_open_device", [name]),
  call: (name) => i18n.t("kdeconnect_call_device", [name]),
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
