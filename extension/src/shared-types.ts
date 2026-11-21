/**
 * Extension-only types that are not part of the native-messaging protocol.
 *
 * Wire types (`Message`, `Device`, `ShareRequest`, …) are generated from Rust
 * in `generated/`; these two are UI/storage shapes that the host never sees.
 */

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
