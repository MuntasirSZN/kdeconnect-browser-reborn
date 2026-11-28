#!/usr/bin/env bun
/**
 * Scripted native-messaging host for E2E tests.
 *
 * Speaks the extension's 4-byte little-endian length-prefixed JSON framing.
 * Incoming frames are logged as JSONL to `FAKE_HOST_LOG`; replies stand in for
 * what `kdeconnectd` would send, so the extension can be exercised without a
 * real daemon or phone.
 *
 * The reply mode is read from `FAKE_HOST_MODE_FILE` on every frame so a test
 * can change behaviour without relaunching the browser. `FAKE_HOST_MODE` is
 * the fallback when no mode file is configured.
 */

import { appendFileSync, readFileSync } from "node:fs";

const PROTOCOL_VERSION = "1.0.4";

const LOG = process.env.FAKE_HOST_LOG;
const MODE_FILE = process.env.FAKE_HOST_MODE_FILE;
const DEFAULT_MODE = process.env.FAKE_HOST_MODE ?? "ok";

interface Frame {
  type?: string;
  data?: unknown;
}

const phone = {
  id: "dev1",
  name: "Pixel E2E",
  type: "phone",
  iconName: "smartphone",
  statusIconName: "smartphoneconnected",
  isReachable: true,
  isTrusted: true,
  supportedPlugins: { kdeconnect_share: true },
};

function mode(): string {
  if (!MODE_FILE) return DEFAULT_MODE;
  try {
    return readFileSync(MODE_FILE, "utf8").trim() || DEFAULT_MODE;
  } catch {
    return DEFAULT_MODE;
  }
}

function logFrame(frame: Frame): void {
  if (LOG) appendFileSync(LOG, `${JSON.stringify(frame)}\n`);
}

function send(frame: Frame, then?: () => void): void {
  const payload = Buffer.from(JSON.stringify(frame), "utf8");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(payload.length, 0);
  process.stdout.write(Buffer.concat([header, payload]), then);
}

function handle(frame: Frame): void {
  const current = mode();

  switch (frame.type) {
    case "typeVersion":
      send({
        type: "typeVersion",
        data: current === "mismatch" ? "9.9.9" : PROTOCOL_VERSION,
      });
      break;

    case "typeDevices":
      send({ type: "typeDevices", data: { dev1: phone } });
      if (current === "error") {
        // Error mode also drops the host so the extension has to reconnect.
        send({ type: "typeError", data: "kdeconnectd not running" }, () => process.exit(0));
      }
      break;

    case "typeShare":
      send({ type: "typeShareResult", data: { ok: true } });
      break;

    default:
      break;
  }
}

let buffered = Buffer.alloc(0);

process.stdin.on("data", (chunk: Buffer) => {
  buffered = Buffer.concat([buffered, chunk]);
  while (buffered.length >= 4) {
    const length = buffered.readUInt32LE(0);
    if (buffered.length < 4 + length) break;
    const body = buffered.subarray(4, 4 + length).toString("utf8");
    buffered = buffered.subarray(4 + length);
    try {
      const frame = JSON.parse(body) as Frame;
      logFrame(frame);
      handle(frame);
    } catch {
      // Ignore malformed frames the way a real host would.
    }
  }
});

process.stdin.on("end", () => {
  process.exit(0);
});
