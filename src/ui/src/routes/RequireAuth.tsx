import { Navigate, Outlet } from 'react-router-dom';
import { useAuthStore } from '../store/auth';

export function RequireAuth() {
  // Token presence only — deliberately NOT the local exp check. A stale exp must
  // never bounce an offline user to a login screen that can't work anyway; actual
  // expiry/revocation is enforced by the server (401 → client.ts clears the token,
  // which re-renders this guard and redirects).
  const hasToken = useAuthStore((s) => s.token !== null);
  if (!hasToken) return <Navigate to="/login" replace />;
  return <Outlet />;
}
