import { api } from './client';
import { useAuthStore } from '../store/auth';

interface LoginResponse {
  token: string;
}

export async function login(username: string, password: string): Promise<string> {
  const data = await api.post<LoginResponse>('/login', { username, password });
  return data.token;
}

// Exchanges the stored token for a fresh 30-day one and persists it. A 401
// (revoked/expired) is handled centrally in client.ts (logout); network
// failures propagate for the caller to ignore.
export async function refreshToken(): Promise<void> {
  const data = await api.post<LoginResponse>('/refresh_token');
  useAuthStore.getState().login(data.token);
}
