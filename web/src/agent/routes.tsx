import { Navigate, type RouteObject } from 'react-router-dom';

import {
  AgentCommissionsPage,
  AgentConvertStatsPage,
  AgentDashboardPage,
  AgentInviteCodesPage,
  AgentTeamTreePage,
  AgentUsersPage
} from './pages';

export const agentRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  { path: 'dashboard', element: <AgentDashboardPage /> },
  { path: 'users', element: <AgentUsersPage /> },
  { path: 'invite-codes', element: <AgentInviteCodesPage /> },
  { path: 'commissions', element: <AgentCommissionsPage /> },
  { path: 'convert-stats', element: <AgentConvertStatsPage /> },
  { path: 'team-tree', element: <AgentTeamTreePage /> }
];
