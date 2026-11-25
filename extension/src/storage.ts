import { storage } from "#imports";

import type { ExtensionOptions } from "./shared-types";

/** Default options shared by the background and the options page. */
export const DEFAULT_OPTIONS: ExtensionOptions = {
  enabled: true,
  defaultOnly: false,
  defaultDeviceId: null,
  disableContextMenu: false,
};

/** The one durable options item: a typed `sync:options` object. */
export const optionsItem = storage.defineItem<ExtensionOptions>("sync:options", {
  fallback: DEFAULT_OPTIONS,
});

/** Legacy flat sync keys written before the single-item settings migration. */
const LEGACY_KEYS = [
  "sync:enabled",
  "sync:defaultOnly",
  "sync:defaultDeviceId",
  "sync:disableContextMenu",
] as const;

/** Read the saved options from storage, applying the shared defaults. */
export async function loadOptions(): Promise<ExtensionOptions> {
  if ((await storage.getItem("sync:options")) != null) {
    return optionsItem.getValue();
  }

  const migrated = await readLegacyOptions();
  if (migrated) {
    await optionsItem.setValue(migrated);
    await clearLegacyOptions();
  }
  return optionsItem.getValue();
}

/** Persist the options object. */
export async function saveOptions(next: ExtensionOptions): Promise<void> {
  await optionsItem.setValue(next);
}

/** Read the four legacy keys; `null` when none of them are stored. */
async function readLegacyOptions(): Promise<ExtensionOptions | null> {
  const values = await Promise.all(LEGACY_KEYS.map((key) => storage.getItem(key)));
  if (values.every((value) => value == null)) {
    return null;
  }
  return {
    enabled: pick(values[0], DEFAULT_OPTIONS.enabled),
    defaultOnly: pick(values[1], DEFAULT_OPTIONS.defaultOnly),
    defaultDeviceId: pick(values[2], DEFAULT_OPTIONS.defaultDeviceId),
    disableContextMenu: pick(values[3], DEFAULT_OPTIONS.disableContextMenu),
  };
}

async function clearLegacyOptions(): Promise<void> {
  await Promise.all(LEGACY_KEYS.map((key) => storage.removeItem(key)));
}

function pick<T>(value: unknown, fallback: T): T {
  return (value as T | null | undefined) ?? fallback;
}
