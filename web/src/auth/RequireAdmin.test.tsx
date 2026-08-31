import { act, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from './authStore';
import { RequireAdmin } from './RequireAdmin';

function renderRouter(router: ReturnType<typeof createMemoryRouter>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}

describe('RequireAdmin', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            admin_id: 1,
            username: 'admin',
            role_id: 1,
            role_name: '超级管理员',
            permissions: ['*'],
            is_super_admin: true
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } }
        )
      )
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders admin content for admin scope', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'admin', subject: 'admin:1' });
    const router = createMemoryRouter([{ path: '/', element: <RequireAdmin>Admin content</RequireAdmin> }]);

    renderRouter(router);

    expect(await screen.findByText('Admin content')).toBeInTheDocument();
  });

  it('redirects unauthenticated users to login', async () => {
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin>Admin content</RequireAdmin> },
      { path: '/login', element: <div>登录</div> }
    ]);

    renderRouter(router);

    expect(await screen.findByText('登录')).toBeInTheDocument();
  });

  it('redirects to login when the session is cleared after render', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'admin', subject: 'admin:1' });
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin>Admin content</RequireAdmin> },
      { path: '/login', element: <div>登录</div> }
    ]);

    renderRouter(router);
    expect(await screen.findByText('Admin content')).toBeInTheDocument();

    act(() => {
      authStore.clearSession();
    });

    expect(await screen.findByText('登录')).toBeInTheDocument();
  });

  it('redirects non-admin sessions to forbidden page', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'agent', subject: 'agent:1' });
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin>Admin content</RequireAdmin> },
      { path: '/403', element: <div>无权限</div> }
    ]);

    renderRouter(router);

    expect(await screen.findByText('无权限')).toBeInTheDocument();
  });
});
