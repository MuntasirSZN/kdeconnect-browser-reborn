import { describe, expect, test } from "bun:test";
import {
  MAX_RETRIES,
  createRetryState,
  markHandshake,
  nextRetryDelay,
  resetRetry,
} from "./reconnect";

describe("reconnect policy", () => {
  test("never retries without a handshake", () => {
    const state = createRetryState();
    expect(nextRetryDelay(state, 1_000)).toBeNull();
  });

  test("a stable session resets the delay", () => {
    const state = createRetryState();
    state.retryCount = 3;
    markHandshake(state, 1_000);
    expect(nextRetryDelay(state, 6_000)).toBe(1_000);
    expect(state.retryCount).toBe(1);
  });

  test("short sessions back off 1, 2, 4, 8, 15 seconds", () => {
    const state = createRetryState();
    markHandshake(state, 0);
    const delays = [1_000, 2_000, 4_000, 8_000, 15_000].map((_, i) =>
      nextRetryDelay(state, 100 + i * 100),
    );
    expect(delays).toEqual([1_000, 2_000, 4_000, 8_000, 15_000]);
    expect(nextRetryDelay(state, 1_000)).toBeNull();
    expect(state.retryCount).toBe(MAX_RETRIES);
  });

  test("reset clears both fields", () => {
    const state = createRetryState();
    markHandshake(state, 10);
    state.retryCount = 2;
    resetRetry(state);
    expect(state).toEqual({ retryCount: 0, handshakeAt: null });
  });
});
