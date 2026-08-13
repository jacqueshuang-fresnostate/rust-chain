import { Navigate, type RouteObject } from 'react-router-dom';

type AgentPageName =
  | 'AgentCommissionsPage'
  | 'AgentConvertStatsPage'
  | 'AgentDashboardPage'
  | 'AgentInviteCodesPage'
  | 'AgentSubAgentsPage'
  | 'AgentTeamTreePage'
  | 'AgentUsersPage';

// 七个代理端页面同处一个模块，按需加载后它们合并为一个独立 chunk，
// 管理端与登录页不再为代理端代码付出首屏体积。handle.page 保留静态可读的路由绑定。
function agentRoute(path: string, page: AgentPageName): RouteObject {
  return {
    path,
    handle: { page },
    lazy: async () => ({ Component: (await import('./pages'))[page] })
  };
}

export const agentRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  agentRoute('dashboard', 'AgentDashboardPage'),
  agentRoute('users', 'AgentUsersPage'),
  agentRoute('invite-codes', 'AgentInviteCodesPage'),
  agentRoute('commissions', 'AgentCommissionsPage'),
  agentRoute('convert-stats', 'AgentConvertStatsPage'),
  agentRoute('team-tree', 'AgentTeamTreePage'),
  agentRoute('sub-agents', 'AgentSubAgentsPage')
];
