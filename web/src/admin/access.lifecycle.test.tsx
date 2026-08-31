import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../api/client';
import { authStore } from '../auth/authStore';
import { AdminAccessGate, useAdminAccess, type AdminAccess } from './access';

vi.mock('../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/client')>();
  return { ...actual, apiRequest: vi.fn() };
});

const apiRequestMock = vi.mocked(apiRequest);

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function access(username: string, adminId: number): AdminAccess {
  return {
    admin_id: adminId,
    is_super_admin: false,
    permissions: ['dashboard.read'],
    role_id: adminId,
    role_name: '运营员',
    username
  };
}

function AccessSubject() {
  return <div>{useAdminAccess().username}</div>;
}

describe('AdminAccessGate 会话隔离', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    apiRequestMock.mockReset();
  });

  it('权限查询键同时包含 subject 与 generation，旧主体响应不得污染新会话', async () => {
    const first = deferred<AdminAccess>();
    const second = deferred<AdminAccess>();
    apiRequestMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    authStore.setSession({
      accessToken: 'access-a',
      generation: 'generation-a',
      refreshToken: 'refresh-a',
      scope: 'admin',
      subject: 'admin:a'
    });
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <MemoryRouter>
        <QueryClientProvider client={queryClient}>
          <AdminAccessGate>
            <AccessSubject />
          </AdminAccessGate>
        </QueryClientProvider>
      </MemoryRouter>
    );
    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(1));

    act(() => {
      authStore.setSession({
        accessToken: 'access-b',
        generation: 'generation-b',
        refreshToken: 'refresh-b',
        scope: 'admin',
        subject: 'admin:b'
      });
    });
    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(2));
    second.resolve(access('admin-b', 2));
    expect(await screen.findByText('admin-b')).toBeInTheDocument();

    first.resolve(access('admin-a', 1));
    await Promise.resolve();
    expect(screen.getByText('admin-b')).toBeInTheDocument();
    expect(screen.queryByText('admin-a')).not.toBeInTheDocument();
    expect(queryClient.getQueryCache().find({ queryKey: ['admin-access', 'admin:b', 'generation-b'] })).toBeDefined();
    expect(apiRequestMock.mock.calls.every(([path]) => path === '/admin/api/v1/access/me')).toBe(true);
    expect((apiRequestMock.mock.calls[0][1]?.signal as AbortSignal).aborted).toBe(true);
  });
});
