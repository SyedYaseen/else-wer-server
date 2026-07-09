import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface Claims {
  sub: number;
  role: 'admin' | 'user';
  username: string;
  iat: number;
  exp: number;
}

interface AuthState {
  token: string | null;
  claims: Claims | null;
  login: (token: string) => void;
  logout: () => void;
  isAuthenticated: () => boolean;
}

function decodeClaims(token: string): Claims | null {
  try {
    const payload = token.split('.')[1];
    const json = atob(payload.replace(/-/g, '+').replace(/_/g, '/'));
    return JSON.parse(json) as Claims;
  } catch {
    return null;
  }
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      token: null,
      claims: null,
      login: (token) => set({ token, claims: decodeClaims(token) }),
      logout: () => set({ token: null, claims: null }),
      isAuthenticated: () => {
        const { claims } = get();
        return claims !== null && claims.exp * 1000 > Date.now();
      },
    }),
    { name: 'elsewer-auth' },
  ),
);
