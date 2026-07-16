import { fireEvent, render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { authStore } from '../auth/authStore';
import { AgentLayout } from './AgentLayout';

function renderAgentLayout(initialEntry = '/agent/dashboard') {
  const router = createMemoryRouter(
    [
      {
        path: '/agent',
        element: <AgentLayout />,
        children: [
          { path: 'dashboard', element: <div>代理总览内容</div> },
          { path: 'users', element: <div>团队用户内容</div> }
        ]
      },
      { path: '/login', element: <div>登录页</div> }
    ],
    { initialEntries: [initialEntry] }
  );

  return render(<RouterProvider router={router} />);
}

describe('AgentLayout', () => {
  beforeEach(() => {
    authStore.setSession({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:9' });
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('renders the agent navigation labels and subject', () => {
    renderAgentLayout();

    ['总览', '团队用户', '邀请码', '佣金记录', '闪兑统计', '团队树'].forEach((label) => {
      expect(screen.getByRole('menuitem', { name: label })).toBeInTheDocument();
    });
    expect(screen.getByText('agent:9')).toBeInTheDocument();
    expect(screen.getByText('代理总览内容')).toBeInTheDocument();
  });

  it('navigates between agent menu items', async () => {
    renderAgentLayout();

    fireEvent.click(screen.getByRole('menuitem', { name: '团队用户' }));

    expect(await screen.findByText('团队用户内容')).toBeInTheDocument();
  });

  it('clears only the agent session on logout', async () => {
    renderAgentLayout();

    fireEvent.click(screen.getByRole('button', { name: /退出登录/ }));

    expect(await screen.findByText('登录页')).toBeInTheDocument();
    expect(authStore.getSession()).toEqual({
      accessToken: 'admin-token',
      refreshToken: 'admin-refresh',
      scope: 'admin',
      subject: 'admin:1'
    });
    expect(authStore.getSession('agent')).toBeNull();
  });
});
