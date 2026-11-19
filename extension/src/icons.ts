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
