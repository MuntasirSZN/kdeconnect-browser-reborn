import type { Message } from "./generated/types.js";

/**
 * Maximum image size, in raw bytes, transferred through one native message.
 * Base64 expands by 4/3, so this keeps the frame well under the host's
 * 1 MiB native-messaging limit; larger images fall back to sharing the URL.
 */
export const MAX_IMAGE_BYTES = 512 * 1024;

/** Whether an image of `size` bytes fits in a single share message. */
export function imageBytesAllowed(size: number): boolean {
  return size > 0 && size <= MAX_IMAGE_BYTES;
}

export interface ImageClickInfo {
  mediaType?: string;
  linkUrl?: string;
  srcUrl?: string;
}

/**
 * Image bytes are only transferred when the click targets the image itself.
 * For an image inside a link the user asked for the link, matching PBI.
 */
export function wantsImageData(info: ImageClickInfo): boolean {
  return info.mediaType === "image" && !info.linkUrl;
}

/**
 * Build the `typeShare` message posted by views and the background.
 * `imageData` carries base64-encoded image bytes for image-context clicks —
 * the host writes them to a temp file and shares that instead of the URL.
 */
export function shareRequest(target: string, url: string, imageData?: string | null): Message {
  return {
    id: crypto.randomUUID(),
    type: "typeShare",
    data: { target, url, imageData: imageData ?? null },
  };
}
