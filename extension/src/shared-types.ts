/**
 * Extension-only types that are not part of the native-messaging protocol.
 *
 * Wire types (`Message`, `Device`, `ShareRequest`, …) are generated from Rust
 * in `generated/`; these two are UI/storage shapes that the host never sees.
 */

/** Options stored in `chrome.storage.sync` by the options page. */
export interface ExtensionOptions {
  /** Whether the extension is active (pops, menus, sharing). */
  enabled: boolean;
  /** When true, only `defaultDeviceId` gets menu entries. */
  defaultOnly: boolean;
  /** Device id to use when `defaultOnly` is set. */
  defaultDeviceId: string | null;
  /** When true, hide the context menu (share via popup instead). */
  disableContextMenu: boolean;
}

/** Status message from the service worker to popup/options views. */
export interface StatusData {
  /** The status kind being reported. */
  type: "typeError" | "typeVersion" | "typeClearStatus" | "typeShareResult";
  /** Status key, e.g. `connected` or `share`. */
  key: string;
  /** Error text for `typeError`. */
  error?: string;
  /** Message text for `typeShareResult`. */
  message?: string;
  /** Remote version for `typeVersion`. */
  update?: string;
  /** Current version for `typeVersion`. */
  current?: string;
}
