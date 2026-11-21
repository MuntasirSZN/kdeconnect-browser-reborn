import type { Device, ExtensionOptions } from "./generated/types.js";
import { deviceMenuIcon } from "./icons.js";

export type MenuAction = "open-link" | "open-page" | "call" | "open-tab";

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
  call: (name: string) => string;
}

export interface MenuClickInfo {
  linkUrl?: string;
  srcUrl?: string;
  pageUrl?: string;
}

const HTTP_PATTERNS = ["http://*/*", "https://*/*"];
const ACTIONS: readonly MenuAction[] = ["open-link", "open-page", "call", "open-tab"];

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
    case "call":
      return {
        id,
        title,
        ...common,
        contexts: ["link"],
        targetUrlPatterns: ["tel:*"],
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

/** Devices missing the share plugin cannot receive anything. */
function canShare(device: Device): boolean {
  return Object.hasOwn(device.supportedPlugins ?? {}, "kdeconnect_share");
}

/** PBI offers the call action to phone-like devices only. */
function canCall(device: Device): boolean {
  return !["desktop", "tv", "laptop"].includes(device.type);
}

/** Flat KDE Connect menus, following PBI's link/page/call actions and Firefox tab action. */
export function buildMenuSpec(
  devices: Record<string, Device>,
  options: ExtensionOptions,
  strings: MenuStrings,
  isFirefox = false,
): MenuEntry[] {
  if (!options.enabled || options.disableContextMenu) return [];

  let usable = Object.values(devices).filter((d) => d.isReachable && d.isTrusted && canShare(d));
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
    if (canCall(device)) {
      entries.push(entryFor("call", device, strings.call(device.name), isFirefox));
    }
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

/** Parse-based http(s) check; unlike `startsWith`, it accepts mixed-case schemes. */
function isWebUrl(value: string): boolean {
  try {
    const parsed = new URL(value);
    return (parsed.protocol === "http:" || parsed.protocol === "https:") && parsed.hostname !== "";
  } catch {
    return false;
  }
}

function isTelUrl(value: string): boolean {
  return /^tel:[^\s]+$/i.test(value);
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
        : action === "call"
          ? info.linkUrl
          : (info.linkUrl ?? info.srcUrl);
  if (!candidate) return null;

  if (action === "call") return isTelUrl(candidate) ? candidate : null;
  return isWebUrl(candidate) ? candidate : null;
}
