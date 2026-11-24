/**
 * Playwright harness: launches Chromium with the built MV3 extension and a
 * scripted native-messaging host that stands in for `kdeconnectd`.
 */

import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium, type BrowserContext, type Page, type Worker } from "@playwright/test";

const HERE = dirname(fileURLToPath(import.meta.url));
const EXTENSION_DIR = join(HERE, "..", ".output", "chrome-mv3");
const HOST_SCRIPT = join(HERE, "fake-host.ts");
const KEY_FILE = join(HERE, "fixtures", "extension-key.pub");
const HOST_NAME = "io.github.muntasirszn.kdeconnect_browser_reborn";

/** Chrome extension id for a base64 DER public key. */
export function extensionIdFromPublicKey(base64: string): string {
  const der = Buffer.from(base64, "base64");
  const digest = createHash("sha256").update(der).digest("hex").slice(0, 32);
  const id = digest
    .split("")
    .map((c) => String.fromCharCode(97 + parseInt(c, 16)))
    .join("");
  if (id.length !== 32) throw new Error(`bad extension id: ${id}`);
  return id;
}

export function readPublicKey(): string {
  return readFileSync(KEY_FILE, "utf8").trim();
}

export type HostMode = "ok" | "mismatch" | "error";

export interface LaunchOptions {
  mode?: HostMode;
  /** Extra environment variables for the browser (and therefore the host). */
  env?: Record<string, string>;
}

export interface Harness {
  context: BrowserContext;
  extensionId: string;
  userDataDir: string;
  hostLog: string;
  popupUrl: string;
  optionsUrl: string;
  background: Worker;
  /** Change what the scripted host replies with, without relaunching. */
  setMode(mode: HostMode): void;
  openPopup(): Promise<Page>;
  openOptions(): Promise<Page>;
  close(): Promise<void>;
}

function writeHostManifest(dir: string, extensionId: string): void {
  const hostManifest = {
    name: HOST_NAME,
    description: "KDE Connect Browser Reborn E2E host",
    path: HOST_SCRIPT,
    type: "stdio",
    allowed_origins: [`chrome-extension://${extensionId}/`],
  };
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${HOST_NAME}.json`), JSON.stringify(hostManifest, null, 2));
}

function stringEnv(source: NodeJS.ProcessEnv): Record<string, string> {
  return Object.fromEntries(
    Object.entries(source).filter((entry): entry is [string, string] => entry[1] !== undefined),
  );
}

/** Launch the extension context. Caller must `close()` it. */
export async function launch(options: LaunchOptions = {}): Promise<Harness> {
  const { mode = "ok", env = {} } = options;

  const userDataDir = mkdtempSync(join(tmpdir(), "kde-e2e-"));
  const hostLog = join(userDataDir, "host.jsonl");
  const modeFile = join(userDataDir, "host-mode.txt");
  writeFileSync(hostLog, "");
  writeFileSync(modeFile, mode);
  chmodSync(HOST_SCRIPT, 0o755);

  const expectedId = extensionIdFromPublicKey(readPublicKey());

  // User-level Chromium native hosts live under $XDG_CONFIG_HOME/chromium.
  const configHome = mkdtempSync(join(tmpdir(), "kde-e2e-xdg-"));
  writeHostManifest(join(configHome, "chromium", "NativeMessagingHosts"), expectedId);
  // Belt and braces for builds that look next to the profile instead.
  writeHostManifest(join(userDataDir, "NativeMessagingHosts"), expectedId);

  const context = await chromium.launchPersistentContext(userDataDir, {
    channel: "chromium",
    headless: true,
    env: {
      ...stringEnv(process.env),
      ...env,
      XDG_CONFIG_HOME: configHome,
      FAKE_HOST_LOG: hostLog,
      FAKE_HOST_MODE_FILE: modeFile,
    },
    args: [
      `--disable-extensions-except=${EXTENSION_DIR}`,
      `--load-extension=${EXTENSION_DIR}`,
    ],
  });

  let background: Worker;
  try {
    background = context.serviceWorkers()[0] ?? (await context.waitForEvent("serviceworker"));
  } catch (error) {
    await context.close();
    throw error;
  }

  const extensionId = new URL(background.url()).host;
  if (extensionId !== expectedId) {
    await context.close();
    throw new Error(`extension id mismatch: launched ${extensionId}, expected ${expectedId}`);
  }

  return {
    context,
    extensionId,
    userDataDir,
    hostLog,
    popupUrl: `chrome-extension://${extensionId}/popup.html`,
    optionsUrl: `chrome-extension://${extensionId}/options.html`,
    background,
    setMode(next) {
      writeFileSync(modeFile, next);
    },
    async openPopup() {
      const page = await context.newPage();
      await page.goto(`chrome-extension://${extensionId}/popup.html`);
      return page;
    },
    async openOptions() {
      const page = await context.newPage();
      await page.goto(`chrome-extension://${extensionId}/options.html`);
      return page;
    },
    close: () => context.close(),
  };
}

/** Read the JSONL native-host log, tolerating a partially-written last line. */
export function readHostLog(path: string): Array<Record<string, unknown>> {
  return readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .flatMap((line) => {
      try {
        return [JSON.parse(line) as Record<string, unknown>];
      } catch {
        return [];
      }
    });
}
