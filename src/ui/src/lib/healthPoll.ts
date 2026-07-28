import { useNetworkStatusStore } from '../store/networkStatus';

// A dedicated raw fetch rather than api/client.ts's api.get(): that wrapper's
// 8s timeout is tuned for data calls (too long for a liveness probe) and its
// auth-header attachment / strict JSON-body parsing are irrelevant to an
// unauthenticated {status:"ok"} probe.
const HEALTH_PATH = '/api/health';
const PROBE_TIMEOUT_MS = 2_500;
const ONLINE_POLL_INTERVAL_MS = 15_000;
const OFFLINE_POLL_INTERVAL_MS = 3_000;
const FAILURE_THRESHOLD = 2;

let consecutiveFailures = 0;
let timerId: ReturnType<typeof setTimeout> | null = null;
let started = false;

async function probe(): Promise<boolean> {
  const controller = new AbortController();
  const t = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);
  try {
    const res = await fetch(HEALTH_PATH, { signal: controller.signal, cache: 'no-store' });
    return res.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(t);
  }
}

function scheduleNext(delayMs: number) {
  if (timerId !== null) clearTimeout(timerId);
  if (document.hidden) {
    // Resumed immediately by the visibilitychange listener below instead of
    // burning cycles polling a backgrounded tab.
    timerId = null;
    return;
  }
  timerId = setTimeout(tick, delayMs);
}

async function tick() {
  const ok = await probe();
  const { reachable, setReachable } = useNetworkStatusStore.getState();
  if (ok) {
    consecutiveFailures = 0;
    if (!reachable) setReachable(true); // instant recovery, no debounce
    scheduleNext(ONLINE_POLL_INTERVAL_MS);
  } else {
    consecutiveFailures += 1;
    if (consecutiveFailures >= FAILURE_THRESHOLD && reachable) setReachable(false);
    // Tighten cadence starting at failure #1, not only after the visible
    // state has flipped, so the confirming check isn't delayed by a stale
    // "still online" interval.
    scheduleNext(OFFLINE_POLL_INTERVAL_MS);
  }
}

document.addEventListener('visibilitychange', () => {
  if (started && document.visibilityState === 'visible') {
    if (timerId !== null) clearTimeout(timerId);
    timerId = null;
    void tick();
  }
});

export function startHealthPolling(): void {
  if (started) return; // idempotent
  started = true;
  void tick();
}
