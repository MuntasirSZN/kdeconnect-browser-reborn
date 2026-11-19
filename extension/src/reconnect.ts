/**
 * Bounded reconnect policy for the native messaging port.
 *
 * The port is retried only after a session has completed its handshake, so a
 * missing host does not spin. Short-lived sessions back off exponentially,
 * while a session that stayed up long enough resets the counter.
 */

export const MAX_RETRIES = 5;
export const STABLE_SESSION_MS = 5_000;

const BASE_DELAY_MS = 1_000;
const MAX_DELAY_MS = 15_000;

export interface RetryState {
  retryCount: number;
  handshakeAt: number | null;
}

export function createRetryState(): RetryState {
  return { retryCount: 0, handshakeAt: null };
}

/** Record that the host has answered the handshake. */
export function markHandshake(state: RetryState, now: number): void {
  state.handshakeAt = now;
}

/**
 * Delay before the next connection attempt, or `null` when no further
 * attempt should be made.
 */
export function nextRetryDelay(state: RetryState, now: number): number | null {
  if (state.handshakeAt === null) {
    return null;
  }

  if (now - state.handshakeAt >= STABLE_SESSION_MS) {
    state.retryCount = 0;
  }

  if (state.retryCount >= MAX_RETRIES) {
    return null;
  }

  const delay = Math.min(BASE_DELAY_MS * 2 ** state.retryCount, MAX_DELAY_MS);
  state.retryCount += 1;
  return delay;
}

/** Forget backoff history, e.g. after a user action. */
export function resetRetry(state: RetryState): void {
  state.retryCount = 0;
  state.handshakeAt = null;
}
