import { createBrowserRouter, Navigate } from 'react-router-dom';

import { adminRoutes } from '../admin/routes';
import { agentRoutes } from '../agent/routes';
import { LoginPage } from '../auth/LoginPage';
import { ForbiddenPage } from '../pages/ForbiddenPage';
import { NotFoundPage } from '../pages/NotFoundPage';

// 登录页、403 与 404 是未登录访客的唯一入口，保持静态导入以避免首屏多一次往返；
// 两套控制台的鉴权守卫、布局与全部子页面都按需加载，登录时不会下载任何控制台代码。
export const router = createBrowserRouter([
  { path: '/', element: <Navigate to="/login" replace /> },
  { path: '/login', element: <LoginPage /> },
  { path: '/403', element: <ForbiddenPage /> },
  {
    path: '/admin',
    lazy: async () => {
      const [{ RequireAdmin }, { AdminLayout }] = await Promise.all([
        import('../auth/RequireAdmin'),
        import('../layouts/AdminLayout')
      ]);

      return {
        Component: function AdminShell() {
          return (
            <RequireAdmin>
              <AdminLayout />
            </RequireAdmin>
          );
        }
      };
    },
    children: adminRoutes
  },
  {
    path: '/agent',
    lazy: async () => {
      const [{ RequireAgent }, { AgentLayout }] = await Promise.all([
        import('../auth/RequireAgent'),
        import('../layouts/AgentLayout')
      ]);

      return {
        Component: function AgentShell() {
          return (
            <RequireAgent>
              <AgentLayout />
            </RequireAgent>
          );
        }
      };
    },
    children: agentRoutes
  },
  { path: '*', element: <NotFoundPage /> }
]);
