import { Navigate, Outlet } from 'react-router-dom';
import { useAuthStore } from '../store/auth';

export function RequireAdmin() {
  const isAdmin = useAuthStore((s) => s.isAdmin());
  if (!isAdmin) return <Navigate to="/" replace />;
  return <Outlet />;
}
