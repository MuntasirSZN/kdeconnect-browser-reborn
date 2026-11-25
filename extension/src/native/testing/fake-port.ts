import type { Message } from "../../generated/types";

type Listener<T> = (value: T) => void;

class FakeEvent<T> {
  private listeners: Listener<T>[] = [];

  addListener(listener: Listener<T>): void {
    this.listeners.push(listener);
  }

  removeListener(listener: Listener<T>): void {
    this.listeners = this.listeners.filter((l) => l !== listener);
  }

  hasListener(listener: Listener<T>): boolean {
    return this.listeners.includes(listener);
  }

  fire(value: T): void {
    for (const listener of this.listeners) listener(value);
  }
}

/** Fake `chrome.runtime.Port` — sees `postMessage`, emits host replies. */
export class FakePort {
  readonly posted: Message[] = [];
  readonly onMessage = new FakeEvent<unknown>();
  readonly onDisconnect = new FakeEvent<void>();
  error?: { message?: string };

  postMessage(message: unknown): void {
    this.posted.push(message as Message);
  }

  emit(message: Message): void {
    this.onMessage.fire(message);
  }

  disconnect(): void {
    this.onDisconnect.fire();
  }
}

/** Last posted message of a type, or `undefined`. */
export function lastPosted(port: FakePort, type: Message["type"]): Message | undefined {
  return port.posted.findLast((m) => m.type === type);
}
