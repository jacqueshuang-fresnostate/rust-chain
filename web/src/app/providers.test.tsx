import { act, render, waitFor } from '@testing-library/react';
import { useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
import { AppProviders } from './providers';

const resetMarketTickerConnectionMock = vi.hoisted(() => vi.fn());

vi.mock('../api/marketTickerSocket', () => ({
  resetMarketTickerConnection: resetMarketTickerConnectionMock
}));

let capturedSignal: AbortSignal | null = null;
let capturedClient: QueryClient | null = null;

function PrivateQueryProbe() {
  capturedClient = useQueryClient();
  useQuery({
    queryKey: ['private-admin-data'],
    queryFn: ({ signal }) => {
      capturedSignal = signal;
      return new Promise(() => undefined);
    }
  });
  return null;
}

describe('AppProviders 身份生命周期', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    capturedClient = null;
    capturedSignal = null;
    resetMarketTickerConnectionMock.mockReset();
    authStore.setSession({
      accessToken: 'admin-access',
      generation: 'generation-a',
      refreshToken: 'admin-refresh',
      scope: 'admin',
      subject: 'admin:7'
    });
  });

  it('登出或跨标签会话变化时取消旧查询、清空私有缓存并重置实时连接', async () => {
    render(
      <AppProviders>
        <PrivateQueryProbe />
      </AppProviders>
    );
    await waitFor(() => expect(capturedSignal).not.toBeNull());
    expect(capturedClient?.getQueryCache().getAll()).toHaveLength(1);
    const oldPrivateQuery = capturedClient?.getQueryCache().getAll()[0];

    act(() => {
      authStore.clearSession('admin', 'generation-a');
    });

    await waitFor(() => expect(capturedSignal?.aborted).toBe(true));
    await waitFor(() => expect(capturedClient?.getQueryCache().getAll()).not.toContain(oldPrivateQuery));
    expect(resetMarketTickerConnectionMock).toHaveBeenCalledTimes(1);
  });
});
