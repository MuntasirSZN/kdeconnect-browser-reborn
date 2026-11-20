import type { Device, ExtensionOptions } from "./generated/types.js";
import { deviceMenuIcon } from "./icons.js";

export type MenuAction = "open-link" | "open-page" | "open-tab";

export interface MenuEntry {
  id: string;
  title: string;
  contexts: [`${chrome.contextMenus.ContextType}`, ...`${chrome.contextMenus.ContextType}`[]];
  targetUrlPatterns?: string[];
  documentUrlPatterns?: string[];
  icons?: { 16: string };
}

export interface MenuStrings {
  open: (name: string) => string;
}

export interface MenuClickInfo {
  linkUrl?: string;
  srcUrl?: string;
  pageUrl?: string;
}

const HTTP_PATTERNS = ["http://*/*", "https://*/*"];
const ACTIONS: readonly MenuAction[] = ["open-link", "open-page", "open-tab"];

function entryFor(
  action: MenuAction,
  device: Device,
  title: string,
  isFirefox: boolean,
): MenuEntry {
  const id = `${action}@${device.id}`;
  const icons = isFirefox ? { 16: deviceMenuIcon(device.type) } : undefined;
  const common = icons ? { icons } : {};

  switch (action) {
    case "open-link":
      return {
        id,
        title,
        ...common,
        contexts: ["link", "image", "audio", "video"],
        targetUrlPatterns: HTTP_PATTERNS,
      };
    case "open-page":
      return {
        id,
        title,
        ...common,
        contexts: ["page"],
        documentUrlPatterns: HTTP_PATTERNS,
      };
    case "open-tab":
      return {
        id,
        title,
        ...common,
        contexts: ["tab"],
        documentUrlPatterns: HTTP_PATTERNS,
      };
  }
}

/** Flat KDE Connect menus, following PBI's link/page actions and Firefox tab action. */
export function buildMenuSpec(
  devices: Record<string, Device>,
  options: ExtensionOptions,
  strings: MenuStrings,
  isFirefox = false,
): MenuEntry[] {
  if (options.disableContextMenu) return [];

  let usable = Object.values(devices).filter((d) => d.isReachable && d.isTrusted);
  if (options.defaultOnly) {
    const id = options.defaultDeviceId;
    if (!id) return [];
    usable = usable.filter((d) => d.id === id);
  }

  const entries: MenuEntry[] = [];
  for (const device of usable) {
    const title = strings.open(device.name);
    entries.push(entryFor("open-link", device, title, isFirefox));
    entries.push(entryFor("open-page", device, title, isFirefox));
    if (isFirefox) entries.push(entryFor("open-tab", device, title, true));
  }
  return entries;
}

export function parseMenuId(id: string): { action: MenuAction; deviceId: string } | null {
  const at = id.indexOf("@");
  if (at <= 0 || at === id.length - 1) return null;
  const action = id.slice(0, at) as MenuAction;
  if (!ACTIONS.includes(action)) return null;
  return { action, deviceId: id.slice(at + 1) };
}

export function resolveMenuUrl(
  action: MenuAction,
  info: MenuClickInfo,
  tabUrl?: string,
): string | null {
  const candidate =
    action === "open-page"
      ? info.pageUrl
      : action === "open-tab"
        ? (info.pageUrl ?? tabUrl)
        : (info.linkUrl ?? info.srcUrl);
  if (!candidate) return null;
  return candidate.startsWith("http://") || candidate.startsWith("https://") ? candidate : null;
}
