/** Shared types that are not derived from Rust. */

/** Options stored in `chrome.storage.sync` by the options page. */
export interface ExtensionOptions {
  enabled: boolean;
  defaultOnly: boolean;
  defaultDeviceId: string | null;
  disableContextMenu: boolean;
}

/** Status message from the service worker to popup/options views. */
export interface StatusData {
  type: "typeError" | "typeVersion" | "typeClearStatus" | "typeShareResult";
  key: string;
  error?: string;
  message?: string;
  update?: string;
  current?: string;
}
