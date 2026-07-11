import { Navigate, Outlet } from 'react-router-dom';
import { useAuthStore } from '../store/auth';

export function RequireOrganize() {
  const canOrganize = useAuthStore((s) => s.canOrganize());
  if (!canOrganize) return <Navigate to="/" replace />;
  return <Outlet />;
}
