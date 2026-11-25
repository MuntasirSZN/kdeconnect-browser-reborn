/**
 * Context-menu rebuilding.
 */

import { browser } from "wxt/browser";

import { i18n } from "#i18n";

import { getDevices } from "../device-cache";
import { getOptions } from "../settings";
import { buildMenuSpec } from "./spec";
import type { MenuStrings } from "./spec";

const IS_FIREFOX = import.meta.env.BROWSER === "firefox";

let menuRebuildInProgress = false;
let menuRebuildQueued = false;

const menuStrings: MenuStrings = {
  open: (name) => i18n.t("kdeconnect_open_device", [name]),
  call: (name) => i18n.t("kdeconnect_call_device", [name]),
};

/** Re-create the context menus from the current device and option state. */
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
        void browser.contextMenus.create(entry, () => {
          // Reading lastError marks a failed create as handled; Chrome otherwise
          // prints an "Unchecked runtime.lastError" warning.
          const error = browser.runtime.lastError;
          if (error) {
            console.debug("context menu create failed:", error.message);
          }
        });
      }

      menuRebuildInProgress = false;
      if (menuRebuildQueued) rebuildContextMenus();
    });
  };

  rebuild();
}
