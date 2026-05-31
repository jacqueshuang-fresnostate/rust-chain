import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';

import { authStore } from './authStore';
import { RequireAdmin } from './RequireAdmin';

describe('RequireAdmin', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('renders admin content for admin scope', () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'admin', subject: 'admin:1' });
    const router = createMemoryRouter([{ path: '/', element: <RequireAdmin>Admin content</RequireAdmin> }]);

    render(<RouterProvider router={router} />);

    expect(screen.getByText('Admin content')).toBeInTheDocument();
  });

  it('redirects unauthenticated users to login', async () => {
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin>Admin content</RequireAdmin> },
      { path: '/login', element: <div>登录</div> }
    ]);

    render(<RouterProvider router={router} />);

    expect(await screen.findByText('登录')).toBeInTheDocument();
  });

  it('redirects non-admin sessions to forbidden page', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'agent', subject: 'agent:1' });
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin>Admin content</RequireAdmin> },
      { path: '/403', element: <div>无权限</div> }
    ]);

    render(<RouterProvider router={router} />);

    expect(await screen.findByText('无权限')).toBeInTheDocument();
  });
});
