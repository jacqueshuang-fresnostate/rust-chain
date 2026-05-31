import { createBrowserRouter, Navigate } from 'react-router-dom';

import { adminRoutes } from '../admin/routes';
import { LoginPage } from '../auth/LoginPage';
import { RequireAdmin } from '../auth/RequireAdmin';
import { AdminLayout } from '../layouts/AdminLayout';
import { ForbiddenPage } from '../pages/ForbiddenPage';
import { NotFoundPage } from '../pages/NotFoundPage';

export const router = createBrowserRouter([
  { path: '/', element: <Navigate to="/login" replace /> },
  { path: '/login', element: <LoginPage /> },
  { path: '/403', element: <ForbiddenPage /> },
  {
    path: '/admin',
    element: (
      <RequireAdmin>
        <AdminLayout />
      </RequireAdmin>
    ),
    children: adminRoutes
  },
  { path: '*', element: <NotFoundPage /> }
]);
