import { useAuthStore } from '../store/auth';
import { useNetworkStatusStore } from '../store/networkStatus';

const BASE_URL = '/api';

const UNAUTHENTICATED_PATHS = new Set(['/login']);

// Bare fetch() has no timeout by default, which can leave a hung request
// (e.g. DNS unreachable off-wifi) stuck forever instead of rejecting so
// callers' offline fallbacks (getOfflineBookData, getLocalProgress) kick in.
const NETWORK_TIMEOUT_MS = 8000;

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set('Content-Type', 'application/json');

  const token = useAuthStore.getState().token;
  if (token && !UNAUTHENTICATED_PATHS.has(path)) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), NETWORK_TIMEOUT_MS);

  let res: Response;
  try {
    res = await fetch(`${BASE_URL}${path}`, { ...options, headers, signal: controller.signal });
  } catch (err) {
    // fetch() itself threw: no response reached us at all (offline, DNS
    // failure, or our own timeout abort) — as opposed to the server
    // responding with a 4xx/5xx, which throws ApiError below instead and is
    // not a reachability signal. healthPoll.ts still owns periodic
    // recovery/its own failure-threshold debounce; this just informs the
    // shared store immediately on a real request failure.
    if (useNetworkStatusStore.getState().reachable) {
      useNetworkStatusStore.getState().setReachable(false);
    }
    throw err;
  } finally {
    clearTimeout(timeoutId);
  }

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    // A real server 401 (expired or revoked token — only ever seen while online,
    // an unreachable server throws above instead) clears the session; RequireAuth
    // reacts to the emptied store and redirects to /login. Login's own 401
    // (bad credentials) is excluded so it stays a normal form error.
    if (res.status === 401 && !UNAUTHENTICATED_PATHS.has(path)) {
      useAuthStore.getState().logout();
    }
    throw new ApiError(res.status, body || res.statusText);
  }

  if (res.status === 204) return undefined as T;

  // A response can arrive with res.ok=true but an empty body if the
  // connection is torn down mid-disconnect (e.g. WiFi dropping) right after
  // headers land — parse defensively so that surfaces as a normal ApiError
  // callers already catch, not a raw JSON.parse SyntaxError.
  const text = await res.text();
  if (!text) throw new ApiError(res.status, 'Empty response body');
  try {
    return JSON.parse(text) as T;
  } catch {
    throw new ApiError(res.status, 'Invalid JSON response');
  }
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'PUT',
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }),
};
