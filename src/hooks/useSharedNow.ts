import { useEffect, useState } from "react";

/**
 * Module-level shared clock: at most one setInterval for the whole app,
 * regardless of how many components call useSharedNow(true).
 */
let sharedNowMs = Date.now();
const listeners = new Set<(nowMs: number) => void>();
let intervalId: number | null = null;

function startSharedClock(): void {
  if (intervalId !== null) {
    return;
  }
  sharedNowMs = Date.now();
  intervalId = window.setInterval(() => {
    sharedNowMs = Date.now();
    listeners.forEach((listener) => {
      listener(sharedNowMs);
    });
  }, 1000);
}

function stopSharedClockIfIdle(): void {
  if (listeners.size > 0 || intervalId === null) {
    return;
  }
  window.clearInterval(intervalId);
  intervalId = null;
}

function subscribeSharedNow(listener: (nowMs: number) => void): () => void {
  listeners.add(listener);
  startSharedClock();
  return () => {
    listeners.delete(listener);
    stopSharedClockIfIdle();
  };
}

/**
 * Returns a second-resolution timestamp that updates once per second while
 * `enabled` is true. When disabled, returns a stable snapshot and does not
 * subscribe to the shared interval.
 */
export function useSharedNow(enabled: boolean): number {
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    if (!enabled) {
      return;
    }

    setNowMs(Date.now());
    return subscribeSharedNow(setNowMs);
  }, [enabled]);

  return nowMs;
}
