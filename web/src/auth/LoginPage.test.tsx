import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

import { adminLogin, adminLoginTwoFactor, getLoginConfig } from '../api/adminAuth';
import { agentLogin } from '../api/agentAuth';
import { authStore } from './authStore';
import { LoginPage } from './LoginPage';

vi.mock('../api/adminAuth', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/adminAuth')>()),
  adminLogin: vi.fn(),
  adminLoginTwoFactor: vi.fn(),
  getLoginConfig: vi.fn(),
}));

vi.mock('../api/agentAuth', () => ({
  agentLogin: vi.fn()
}));

const adminLoginMock = vi.mocked(adminLogin);
const adminLoginTwoFactorMock = vi.mocked(adminLoginTwoFactor);
const agentLoginMock = vi.mocked(agentLogin);
const getLoginConfigMock = vi.mocked(getLoginConfig);

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
    delete window.turnstile;
    document.querySelectorAll('script[src*="challenges.cloudflare.com/turnstile"]').forEach((script) => script.remove());
    adminLoginMock.mockReset();
    adminLoginTwoFactorMock.mockReset();
    agentLoginMock.mockReset();
    getLoginConfigMock.mockResolvedValue({
      usernameLoginEnabled: true,
      cfTurnstileEnabled: false,
      cfTurnstileSiteKey: '',
    });
  });

  it('renders the runtime-configured Turnstile widget and submits its token', async () => {
    const user = userEvent.setup();
    let widgetOptions: Record<string, unknown> | undefined;
    const renderWidget = vi.fn((_element: string | HTMLElement, options: Record<string, unknown>) => {
      widgetOptions = options;
      return 'widget-1';
    });
    window.turnstile = {
      render: renderWidget,
      reset: vi.fn(),
      remove: vi.fn(),
    };
    getLoginConfigMock.mockResolvedValueOnce({
      usernameLoginEnabled: true,
      cfTurnstileEnabled: true,
      cfTurnstileSiteKey: 'runtime-site-key',
    });
    adminLoginMock.mockResolvedValueOnce({
      access_token: 'admin-access',
      refresh_token: 'admin-refresh',
      token_type: 'Bearer',
      scope: 'admin',
      subject: 'admin:7',
    });

    renderLoginPage();

    await waitFor(() => {
      expect(renderWidget).toHaveBeenCalledWith(
        expect.objectContaining({ className: 'admin-login-turnstile-widget' }),
        expect.objectContaining({ sitekey: 'runtime-site-key' }),
      );
    });
    expect(getLoginConfigMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      (widgetOptions?.callback as ((token: string) => void) | undefined)?.('turnstile-token');
    });
    await user.type(screen.getByLabelText('管理员账号'), 'admin');
    await user.type(screen.getByLabelText('密码'), 'password');
    await user.click(screen.getByRole('button', { name: '登录' }));

    await waitFor(() => {
      expect(adminLoginMock).toHaveBeenCalledWith({
        username: 'admin',
        password: 'password',
        cf_turnstile_token: 'turnstile-token',
      });
    });
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
    expect(screen.getByAltText('HIPPO')).toBeInTheDocument();
    expect(screen.queryByText('HIPPO OPERATIONS')).not.toBeInTheDocument();
    expect(screen.getByText('生产环境')).toBeInTheDocument();
    expect(screen.getByText('安全访问')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '管理员登录' })).toBeInTheDocument();
    expect(document.title).toBe('登录 · HIPPO 管理后台');
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

  it('prompts for the admin two-factor code before storing a session', async () => {
    const user = userEvent.setup();
    adminLoginMock.mockResolvedValueOnce({
      requires_2fa: true,
      challenge_id: 'challenge-1',
      expires_in_seconds: 300
    });
    adminLoginTwoFactorMock.mockResolvedValueOnce({
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

    const codeInput = await screen.findByLabelText('两步验证码');
    expect(authStore.getSession()).toBeNull();

    await user.type(codeInput, '123456');
    await user.click(screen.getByRole('button', { name: '验证并登录' }));

    await waitFor(() => {
      expect(adminLoginTwoFactorMock).toHaveBeenCalledWith({ challenge_id: 'challenge-1', totp_code: '123456' });
    });
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
