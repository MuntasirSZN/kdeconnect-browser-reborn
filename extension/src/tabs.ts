import { browser } from "wxt/browser";

/** URL of the active tab in the current window, or null when unavailable. */
export async function getCurrentTabUrl(): Promise<string | null> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  return tab?.url ?? null;
}
