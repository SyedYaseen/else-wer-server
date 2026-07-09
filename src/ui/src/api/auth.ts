import { api } from './client';

interface LoginResponse {
  token: string;
}

export async function login(username: string, password: string): Promise<string> {
  const data = await api.post<LoginResponse>('/login', { username, password });
  return data.token;
}
