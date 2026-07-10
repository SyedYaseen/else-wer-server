import { useAuthStore } from '../store/auth';

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
  } finally {
    clearTimeout(timeoutId);
  }

  if (!res.ok) {
    const body = await res.text().catch(() => '');
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
};
