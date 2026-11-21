import type { ExtensionOptions } from "./generated/types.js";

/** Default options shared by the background and the options page. */
export const DEFAULT_OPTIONS: ExtensionOptions = {
  defaultOnly: false,
  defaultDeviceId: null,
  disableContextMenu: false,
};

/** Read the saved options from storage, applying the shared defaults. */
export function getStoredOptions(callback: (items: ExtensionOptions) => void): void {
  chrome.storage.sync.get(DEFAULT_OPTIONS, callback);
}
