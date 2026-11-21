/**
 * Options page state: default device id and the save-status timer.
 */

let defaultDeviceId: string | null = null;
let saveStatusTimeout: ReturnType<typeof setTimeout> | null = null;

/** The device the background should treat as the primary device, if any. */
export function getDefaultDeviceId(): string | null {
  return defaultDeviceId;
}

export function setDefaultDeviceId(id: string | null): void {
  defaultDeviceId = id;
}

export function clearSaveStatusTimer(): void {
  if (saveStatusTimeout !== null) {
    clearTimeout(saveStatusTimeout);
    saveStatusTimeout = null;
  }
}

export function setSaveStatusTimer(timer: ReturnType<typeof setTimeout> | null): void {
  saveStatusTimeout = timer;
}
