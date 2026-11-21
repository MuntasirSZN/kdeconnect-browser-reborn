import type { Message } from "./generated/types.js";

/**
 * Build the `typeShare` message posted by views and the background.
 * `imageData` carries base64-encoded image bytes for image-context clicks —
 * the host writes them to a temp file and shares that instead of the URL.
 */
export function shareRequest(target: string, url: string, imageData?: string | null): Message {
  return { type: "typeShare", data: { target, url, imageData: imageData ?? null } };
}
