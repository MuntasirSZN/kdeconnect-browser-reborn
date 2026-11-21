/**
 * Popup view — status-line rendering.
 */

import type { Message } from "../generated/types";
import type { StatusData } from "../shared-types";

/** Add or update the status line for `data.key`. */
export function addStatus(msg: Message): void {
  const data = msg.data as StatusData | undefined;
  if (!data?.key) return;

  const container = document.getElementById("status");
  if (!container) return;

  let el = document.getElementById(`status-${data.key}`);
  if (!el) {
    el = document.createElement("p");
    el.id = `status-${data.key}`;
    el.className = "status";
    container.appendChild(el);
  }

  el.innerHTML = "";

  switch (data.type) {
    case "typeVersion": {
      el.textContent = `Host upgrade v${String(data.update)} available (current: v${String(data.current)}). `;
      const link = document.createElement("a");
      link.href = "https://github.com/MuntasirSZN/kdeconnect-browser-reborn#upgrading";
      link.target = "_blank";
      link.textContent = "Upgrade instructions";
      el.appendChild(link);
      break;
    }
    case "typeError":
      el.className = "status error";
      el.textContent = `Error: ${String(data.error ?? "unknown")}`;
      break;
    case "typeShareResult":
      el.className = "status";
      el.textContent = String(data.message ?? "Sent");
      break;
    default:
      el.textContent = String(data.error ?? "");
  }
}

/** Remove the status line for `data.key`. */
export function removeStatus(msg: Message): void {
  const data = msg.data as StatusData | undefined;
  const key = data?.key;
  if (!key) return;
  const el = document.getElementById(`status-${key}`);
  el?.remove();
}
