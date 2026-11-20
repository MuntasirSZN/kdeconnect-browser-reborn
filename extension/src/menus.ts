import type { Device, ExtensionOptions } from "./generated/types.js";

export type MenuAction = "open-link" | "open-page" | "call" | "open-tab";

export interface MenuEntry {
  id: string;
  parentId?: string;
  title: string;
  contexts: [`${chrome.contextMenus.ContextType}`, ...`${chrome.contextMenus.ContextType}`[]];
  targetUrlPatterns?: string[];
  documentUrlPatterns?: string[];
}

export interface MenuStrings {
  root: string;
  open: (name: string) => string;
  call: (name: string) => string;
}

export interface MenuClickInfo {
  linkUrl?: string;
  srcUrl?: string;
  pageUrl?: string;
}

const HTTP_PATTERNS = ["http://*/*", "https://*/*"];

const ROOT_ID = "kdeconnect-root";

/** Types that cannot receive a `tel:` link. */
const NO_CALL_TYPES = new Set(["desktop", "laptop", "tv"]);

const ACTIONS: readonly MenuAction[] = ["open-link", "open-page", "call", "open-tab"];

function entryFor(
  action: MenuAction,
  device: Device,
  strings: MenuStrings,
  parentId?: string,
): MenuEntry {
  const id = `${action}@${device.id}`;
  const common = parentId === undefined ? {} : { parentId };

  switch (action) {
    case "open-link":
      return {
        id,
        ...common,
        title: strings.open(device.name),
        contexts: ["link", "image", "audio", "video"],
        targetUrlPatterns: HTTP_PATTERNS,
      };
    case "open-page":
      return {
        id,
        ...common,
        title: strings.open(device.name),
        contexts: ["page"],
        documentUrlPatterns: HTTP_PATTERNS,
      };
    case "call":
      return {
        id,
        ...common,
        title: strings.call(device.name),
        contexts: ["link"],
        targetUrlPatterns: ["tel:*"],
      };
    case "open-tab":
      return {
        id,
        ...common,
        title: strings.open(device.name),
        contexts: ["tab"],
        documentUrlPatterns: HTTP_PATTERNS,
      };
  }
}

/**
 * Build the context-menu tree for the current device snapshot.
 *
 * A single usable device gets flat pbi-style items; two or more are grouped
 * under a `KDE Connect` submenu.
 */
export function buildMenuSpec(
  devices: Record<string, Device>,
  options: ExtensionOptions,
  strings: MenuStrings,
): MenuEntry[] {
  if (options.disableContextMenu) return [];

  let usable = Object.values(devices).filter((d) => d.isReachable && d.isTrusted);

  if (options.defaultOnly) {
    const id = options.defaultDeviceId;
    if (!id) return [];
    usable = usable.filter((d) => d.id === id);
  }

  if (usable.length === 0) return [];

  const grouped = usable.length > 1;
  const entries: MenuEntry[] = [];

  if (grouped) {
    entries.push({
      id: ROOT_ID,
      title: strings.root,
      contexts: ["page", "link", "image", "audio", "video", "tab"],
    });
  }

  for (const device of usable) {
    for (const action of ACTIONS) {
      if (action === "call" && NO_CALL_TYPES.has(device.type)) continue;
      entries.push(entryFor(action, device, strings, grouped ? ROOT_ID : undefined));
    }
  }

  return entries;
}

/** Decode a menu item id into its action and target device. */
export function parseMenuId(id: string): { action: MenuAction; deviceId: string } | null {
  const at = id.indexOf("@");
  if (at <= 0 || at === id.length - 1) return null;

  const action = id.slice(0, at) as MenuAction;
  if (!ACTIONS.includes(action)) return null;

  return { action, deviceId: id.slice(at + 1) };
}

/** Pick the URL to share for a click and reject unsafe schemes. */
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

  if (action === "call") {
    return candidate.startsWith("tel:") ? candidate : null;
  }

  return candidate.startsWith("http://") || candidate.startsWith("https://") ? candidate : null;
}
