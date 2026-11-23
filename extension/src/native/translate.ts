import type { Device, Message } from "../generated/types";

/** Effect produced by translating a native-host message. */
export type HostEffect =
  | { kind: "devices"; devices: Record<string, Device> }
  | { kind: "device"; device: Device }
  | { kind: "error"; key: string; error: string }
  | { kind: "shareResult"; ok: boolean; error?: string }
  | { kind: "version"; remote: string; matches: boolean };

/**
 * Pure translation of a native host message into browser-side effects.
 *
 * The port (`native/port.ts`) is the mutating half: it applies the effects to
 * the device cache and status UI.
 */
export function translateHostMessage(msg: Message, protocolVersion: string): HostEffect[] {
  switch (msg.type) {
    case "typeDevices":
      return [{ kind: "devices", devices: msg.data as Record<string, Device> }];

    case "typeDeviceUpdate":
      return [{ kind: "device", device: msg.data as Device }];

    case "typeError":
      return [{ kind: "error", key: "host", error: msg.data as string }];

    case "typeShareResult": {
      const data = msg.data as { ok?: boolean; error?: string } | undefined;
      const effect: HostEffect =
        data?.ok === true
          ? { kind: "shareResult", ok: true }
          : { kind: "shareResult", ok: false, error: data?.error ?? "share failed" };
      return [effect];
    }

    case "typeVersion": {
      const remote = msg.data as string;
      return [{ kind: "version", remote, matches: remote === protocolVersion }];
    }

    default:
      return [];
  }
}
