/**
 * Background service worker — wires the port, menus, and options modules.
 */

import { defineBackground } from "#imports";
import { browser } from "wxt/browser";

import { getDevices } from "@/device-cache";
import { rebuildContextMenus } from "@/menus/register";
import { ensureConnection, disableIntegration, postShare, handleMenuClick } from "@/native/port";
import { onMessage } from "@/messaging";
import { getOptions, setOptions } from "@/settings";
import { loadOptions, optionsItem } from "@/storage";

export default defineBackground({
  type: "module",
  main() {
    onMessage("connect", () => ensureConnection());
    onMessage("getDevices", () => getDevices());
    onMessage("share", ({ data }) => postShare(data));
    optionsItem.watch((next) => {
      setOptions(next);
      if (!next.enabled) disableIntegration();
      else ensureConnection();
      rebuildContextMenus();
    });
    browser.contextMenus.onClicked.addListener((info, tab) => void handleMenuClick(info, tab));
    void bootstrap();
  },
});

async function bootstrap(): Promise<void> {
  setOptions(await loadOptions());
  if (getOptions().enabled) ensureConnection();
  rebuildContextMenus();
}
