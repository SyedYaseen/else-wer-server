import { useNetworkStatusStore } from '../store/networkStatus';

// A dedicated raw fetch rather than api/client.ts's api.get(): that wrapper's
// 8s timeout is tuned for data calls (too long for a liveness probe), and its
// auth-header attachment / throw-on-bad-JSON parsing are wrong for an
// unauthenticated probe whose body is read best-effort.
const HEALTH_PATH = '/api/health';
const PROBE_TIMEOUT_MS = 2_500;
const ONLINE_POLL_INTERVAL_MS = 15_000;
const OFFLINE_POLL_INTERVAL_MS = 3_000;
const FAILURE_THRESHOLD = 2;

let consecutiveFailures = 0;
let timerId: ReturnType<typeof setTimeout> | null = null;
let started = false;

// Reachability and media state are reported separately: the server answers 200 even
// when a library's disk is unreachable (a non-2xx would just read as "offline" and
// hide the specific cause), so the drive signal has to come out of the body.
interface ProbeResult {
  ok: boolean;
  mediaOk: boolean | null; // null = unknown: older server, unparseable body, or unreachable
}

async function probe(boot = false): Promise<ProbeResult> {
  const controller = new AbortController();
  const t = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);
  try {
    // ?boot=1 on the first probe of a page load only. It changes nothing about the
    // probe; it makes app starts countable in the server's request log, which is
    // the only vantage point available for a standalone iOS PWA — iOS discards a
    // backgrounded PWA and reloads it on the next open, and a reload is
    // indistinguishable from normal traffic without this marker.
    const url = boot ? `${HEALTH_PATH}?boot=1` : HEALTH_PATH;
    const res = await fetch(url, { signal: controller.signal, cache: 'no-store' });
    if (!res.ok) return { ok: false, mediaOk: null };
    // Body parsing is best-effort and deliberately cannot affect `ok`: a server that
    // predates media_ok, or any non-JSON body, leaves the drive state unknown rather
    // than downgrading a perfectly good liveness result to "offline".
    try {
      const body: unknown = await res.json();
      const mediaOk = (body as { media_ok?: unknown } | null)?.media_ok;
      return { ok: true, mediaOk: typeof mediaOk === 'boolean' ? mediaOk : null };
    } catch {
      return { ok: true, mediaOk: null };
    }
  } catch {
    return { ok: false, mediaOk: null };
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

async function tick(boot = false) {
  const { ok, mediaOk } = await probe(boot);
  const {
    reachable,
    setReachable,
    mediaOk: prevMediaOk,
    setMediaOk,
  } = useNetworkStatusStore.getState();
  // No debounce, unlike reachability: this comes from a successful response rather
  // than an absence of one, so there's no flapping to smooth out.
  if (mediaOk !== prevMediaOk) setMediaOk(mediaOk);
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
  void tick(true);
}
