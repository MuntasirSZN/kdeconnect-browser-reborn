// Barrel re-exporting all ts-rs generated types + hand-written shared types.
// This file is NOT auto-generated; keep it in sync when adding/removing types.

export type { Empty } from "./Empty";
export type { Device } from "./Device";
export type { Message } from "./Message";
export type { MessageType } from "./MessageType";
export type { ShareRequest } from "./ShareRequest";

// ---- Hand-written types (not derived from Rust) --------------------------

/** Mirrors Rust's `struct ExtensionOptions` in `messaging.rs`. */
export interface ExtensionOptions {
  defaultOnly: boolean;
  defaultDeviceId: string | null;
  disableContextMenu: boolean;
}

/** Status message from service worker to popup/options. */
export interface StatusData {
  type: "typeError" | "typeVersion" | "typeClearStatus";
  key: string;
  error?: string;
  update?: string;
  current?: string;
}
