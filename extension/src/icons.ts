import { Laptop, Monitor, MonitorSmartphone, Smartphone, Tablet, Tv } from "lucide";

export type IconName = "smartphone" | "tablet" | "laptop" | "monitor" | "tv" | "monitor-smartphone";

export const DEVICE_ICONS = { Laptop, Monitor, MonitorSmartphone, Smartphone, Tablet, Tv };

/** Map a KDE Connect device type to a Lucide icon name. */
export function typeIcon(type: string): IconName {
  switch (type) {
    case "phone":
    case "smartphone":
      return "smartphone";
    case "tablet":
      return "tablet";
    case "laptop":
      return "laptop";
    case "desktop":
      return "monitor";
    case "tv":
      return "tv";
    default:
      return "monitor-smartphone";
  }
}

const DEVICE_ICON_KEYS: Record<IconName, keyof typeof DEVICE_ICONS> = {
  smartphone: "Smartphone",
  tablet: "Tablet",
  laptop: "Laptop",
  monitor: "Monitor",
  tv: "Tv",
  "monitor-smartphone": "MonitorSmartphone",
};

const menuIconCache = new Map<IconName, string>();

function escapeSvgAttribute(value: string | number): string {
  return String(value).replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;");
}

/** Render the existing Lucide icon node as a Firefox context-menu data URL. */
export function deviceMenuIcon(type: string): string {
  const name = typeIcon(type);
  const cached = menuIconCache.get(name);
  if (cached) return cached;

  const icon = DEVICE_ICONS[DEVICE_ICON_KEYS[name]];
  const children = icon
    .map(([tag, attributes]) => {
      const attrs = Object.entries(attributes)
        .filter(([, value]) => value !== undefined)
        .map(([key, value]) => `${key}="${escapeSvgAttribute(String(value))}"`)
        .join(" ");
      return `<${tag} ${attrs}/>`;
    })
    .join("");

  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#4d4d4d" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
    children +
    "</svg>";
  const dataUrl = `data:image/svg+xml,${encodeURIComponent(svg)}`;
  menuIconCache.set(name, dataUrl);
  return dataUrl;
}
