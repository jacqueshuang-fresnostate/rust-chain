import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

import { adminLogin } from '../api/adminAuth';
import { agentLogin } from '../api/agentAuth';
import { authStore } from './authStore';
import { LoginPage } from './LoginPage';

vi.mock('../api/adminAuth', () => ({
  adminLogin: vi.fn()
}));

vi.mock('../api/agentAuth', () => ({
  agentLogin: vi.fn()
}));

const adminLoginMock = vi.mocked(adminLogin);
const agentLoginMock = vi.mocked(agentLogin);

function renderLoginPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const router = createMemoryRouter(
    [
      { path: '/login', element: <LoginPage /> },
      { path: '/admin/dashboard', element: <div>管理员控制台</div> },
      { path: '/agent/dashboard', element: <div>代理控制台</div> }
    ],
    { initialEntries: ['/login'] }
  );

  render(
    <QueryClientProvider client={queryClient}>{<RouterProvider router={router} /> as ReactNode}</QueryClientProvider>
  );
}

describe('LoginPage', () => {
  beforeEach(() => {
    localStorage.clear();
    adminLoginMock.mockReset();
    agentLoginMock.mockReset();
  });

  it('logs in as admin and stores the admin session', async () => {
    const user = userEvent.setup();
    adminLoginMock.mockResolvedValueOnce({
      access_token: 'admin-access',
      refresh_token: 'admin-refresh',
      token_type: 'Bearer',
      scope: 'admin',
      subject: 'admin:7'
    });

    renderLoginPage();
    await user.type(screen.getByLabelText('管理员账号'), 'admin');
    await user.type(screen.getByLabelText('密码'), 'password');
    await user.click(screen.getByRole('button', { name: '登录' }));

    await waitFor(() => {
      expect(adminLoginMock).toHaveBeenCalledWith({ username: 'admin', password: 'password' });
    });
    expect(agentLoginMock).not.toHaveBeenCalled();
    expect(authStore.getSession()).toEqual({
      accessToken: 'admin-access',
      refreshToken: 'admin-refresh',
      scope: 'admin',
      subject: 'admin:7'
    });
    expect(await screen.findByText('管理员控制台')).toBeInTheDocument();
  });

  it('logs in as agent and stores the agent session separately', async () => {
    const user = userEvent.setup();
    authStore.setSession({ accessToken: 'admin-old', refreshToken: 'admin-refresh-old', scope: 'admin', subject: 'admin:1' });
    agentLoginMock.mockResolvedValueOnce({
      access_token: 'agent-access',
      refresh_token: 'agent-refresh',
      token_type: 'Bearer',
      scope: 'agent',
      subject: 'agent:9'
    });

    renderLoginPage();
    await user.click(screen.getByLabelText('代理'));
    await user.type(screen.getByLabelText('代理账号'), 'agent');
    await user.type(screen.getByLabelText('密码'), 'password');
    await user.click(screen.getByRole('button', { name: '登录' }));

    await waitFor(() => {
      expect(agentLoginMock).toHaveBeenCalledWith({ username: 'agent', password: 'password' });
    });
    expect(adminLoginMock).not.toHaveBeenCalled();
    expect(authStore.getSession()).toEqual({ accessToken: 'admin-old', refreshToken: 'admin-refresh-old', scope: 'admin', subject: 'admin:1' });
    expect(authStore.getSession('agent')).toEqual({
      accessToken: 'agent-access',
      refreshToken: 'agent-refresh',
      scope: 'agent',
      subject: 'agent:9'
    });
    expect(await screen.findByText('代理控制台')).toBeInTheDocument();
  });
});
