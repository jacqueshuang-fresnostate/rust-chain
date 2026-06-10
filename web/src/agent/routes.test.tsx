import { isValidElement } from 'react';
import { createMemoryRouter, Navigate, RouterProvider } from 'react-router-dom';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { agentRoutes } from './routes';

function routeElementName(path: string) {
  const route = agentRoutes.find((candidate) => candidate.path === path);
  const element = route?.element;
  return isValidElement(element) && typeof element.type !== 'string' ? String(element.type.name ?? '') : '';
}

describe('agentRoutes', () => {
  it('redirects index route to dashboard', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/agent',
          children: [
            ...agentRoutes,
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
    ['team-tree', 'AgentTeamTreePage']
  ])('registers %s page', (path, expectedName) => {
    expect(routeElementName(path)).toBe(expectedName);
  });
});
