import type { ExtensionOptions } from "./shared-types";

import { DEFAULT_OPTIONS } from "./storage";

let options: ExtensionOptions = { ...DEFAULT_OPTIONS };

/** Current user options. */
export function getOptions(): Readonly<ExtensionOptions> {
  return options;
}

/** Replace the options object. */
export function setOptions(next: ExtensionOptions): void {
  options = next;
}
