import type { Message } from "./generated/types.js";

/** Build the `typeShare` message posted by views and the background. */
export function shareRequest(target: string, url: string): Message {
  return { type: "typeShare", data: { target, url } };
}
