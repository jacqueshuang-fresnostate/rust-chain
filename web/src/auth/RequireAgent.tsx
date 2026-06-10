import type { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';

import { authStore } from './authStore';

export function RequireAgent({ children }: { children: ReactNode }) {
  const location = useLocation();
  const session = authStore.getSession('agent');

  if (session?.scope === 'agent') {
    return <>{children}</>;
  }

  if (session || authStore.getSession()) {
    return <Navigate to="/403" replace />;
  }

  return <Navigate to="/login" replace state={{ from: location.pathname }} />;
}
