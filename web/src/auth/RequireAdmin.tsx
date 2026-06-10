import type { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';

import { authStore } from './authStore';

export function RequireAdmin({ children }: { children: ReactNode }) {
  const location = useLocation();
  const session = authStore.getSession();

  if (!session) {
    if (authStore.getSession('agent')) {
      return <Navigate to="/403" replace />;
    }

    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }

  if (session.scope !== 'admin') {
    return <Navigate to="/403" replace />;
  }

  return <>{children}</>;
}
