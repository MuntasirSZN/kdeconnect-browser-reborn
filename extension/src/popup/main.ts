/**
 * Popup view — entry point.
 */

import type { Message } from "../generated/types";
import { init, onMessage } from "./message";

document.addEventListener("DOMContentLoaded", () => {
  init().catch((e: unknown) => {
    console.warn("popup init failed", e);
  });
});

chrome.runtime.onMessage.addListener((msg: unknown) => {
  onMessage(msg as Message);
});
