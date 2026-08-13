import { isValidElement, type ComponentType } from 'react';
import { createMemoryRouter, Navigate, RouterProvider, type RouteObject } from 'react-router-dom';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { agentRoutes } from './routes';

function findRoute(path: string): RouteObject | undefined {
  return agentRoutes.find((candidate) => candidate.path === path);
}

// 解析 lazy 既验证路由绑定的页面名，也确认目标模块真的导出了该组件。
async function lazyComponentName(path: string) {
  const route = findRoute(path);
  if (typeof route?.lazy !== 'function') {
    return '';
  }

  const resolved = (await route.lazy()) as { Component?: ComponentType };
  return resolved.Component?.name ?? '';
}

describe('agentRoutes', () => {
  it('redirects index route to dashboard', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/agent',
          children: [
            ...agentRoutes.filter((route) => route.index),
            { path: 'dashboard', element: <div>代理总览</div> }
          ]
        }
      ],
      { initialEntries: ['/agent'] }
    );

    render(<RouterProvider router={router} />);

    expect(await screen.findByText('代理总览')).toBeInTheDocument();
    expect(isValidElement(agentRoutes[0].element) && agentRoutes[0].element.type).toBe(Navigate);
  });

  it.each([
    ['dashboard', 'AgentDashboardPage'],
    ['users', 'AgentUsersPage'],
    ['invite-codes', 'AgentInviteCodesPage'],
    ['commissions', 'AgentCommissionsPage'],
    ['convert-stats', 'AgentConvertStatsPage'],
    ['team-tree', 'AgentTeamTreePage'],
    ['sub-agents', 'AgentSubAgentsPage']
  ])(
    // 首个用例要真实转换整份代理端页面模块，并行负载下会超过默认超时。
    'registers %s page',
    async (path, expectedName) => {
      expect(findRoute(path)?.handle).toEqual({ page: expectedName });
      expect(await lazyComponentName(path)).toBe(expectedName);
    },
    120_000
  );

  it('keeps every agent page out of the initial bundle', () => {
    const eagerRoutes = agentRoutes.filter((route) => route.path && route.element);
    expect(eagerRoutes).toEqual([]);
  });
});
