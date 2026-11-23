/**
 * Browser action (toolbar button) helpers: the badge state map and badge rendering.
 */

import { browser } from "wxt/browser";

/** The Chrome action / Firefox browserAction API pair. */
type ActionApi = NonNullable<typeof browser.action> | NonNullable<typeof browser.browserAction>;

interface BadgeState {
  text: string;
  color: [number, number, number, number];
}

const badges: Record<string, BadgeState> = {};

/** Resolve the action API once: Chrome has `action`, Firefox has `browserAction`. */
function actionApi(): ActionApi {
  const api = browser.action ?? browser.browserAction;
  if (!api) {
    throw new Error("browser action API is unavailable in this browser");
  }
  return api;
}

/** Set a badge (used for connect/disconnect/update markers). */
export function setBadge(source: string, state: BadgeState): void {
  badges[source] = state;
  void applyBadge();
}

export function clearBadge(source: string): void {
  delete badges[source];
  void applyBadge();
}

async function applyBadge(): Promise<void> {
  const keys = Object.keys(badges);
  const api = actionApi();
  if (keys.length === 0) {
    await api.setBadgeText({ text: "" });
    return;
  }
  const last = badges[keys[keys.length - 1]];
  if (!last) {
    return;
  }
  await api.setBadgeText({ text: last.text });
  await api.setBadgeBackgroundColor({ color: last.color });
}
