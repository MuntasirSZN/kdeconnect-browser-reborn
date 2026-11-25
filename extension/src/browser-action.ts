/**
 * Browser action (toolbar button) helpers: the badge state map and badge rendering.
 */

import { browser } from "wxt/browser";

/** The Chrome action / Firefox browserAction API pair. */
type ActionApi = NonNullable<typeof browser.action> | NonNullable<typeof browser.browserAction>;

/** State of one badge source: the badge text and its background color. */
interface BadgeState {
  /** Badge text shown in the toolbar button. */
  text: string;
  /** Badge background color as `[r, g, b, a]`. */
  color: [number, number, number, number];
}

/** Live badge states, keyed by the source that set them. */
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

/** Remove the badge set by `source`; the top remaining badge, if any, shows. */
export function clearBadge(source: string): void {
  delete badges[source];
  void applyBadge();
}

/** Render the top badge of all live sources, or clear the badge when none remain. */
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
