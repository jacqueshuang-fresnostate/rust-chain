import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';

import { authStore } from './authStore';
import { RequireAgent } from './RequireAgent';

describe('RequireAgent', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('renders agent content for agent scope', () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'agent', subject: 'agent:1' });
    const router = createMemoryRouter([{ path: '/', element: <RequireAgent>Agent content</RequireAgent> }]);

    render(<RouterProvider router={router} />);

    expect(screen.getByText('Agent content')).toBeInTheDocument();
  });

  it('redirects unauthenticated users to login', async () => {
    const router = createMemoryRouter([
      { path: '/', element: <RequireAgent>Agent content</RequireAgent> },
      { path: '/login', element: <div>登录</div> }
    ]);

    render(<RouterProvider router={router} />);

    expect(await screen.findByText('登录')).toBeInTheDocument();
  });

  it('redirects non-agent sessions to forbidden page', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'admin', subject: 'admin:1' });
    const router = createMemoryRouter([
      { path: '/', element: <RequireAgent>Agent content</RequireAgent> },
      { path: '/403', element: <div>无权限</div> }
    ]);

    render(<RouterProvider router={router} />);

    expect(await screen.findByText('无权限')).toBeInTheDocument();
  });
});
