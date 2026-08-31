import { act, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

import { authStore } from '../auth/authStore';
import { useSharedAdminOptionQuery } from './sharedOptionQuery';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function Consumer({ id, load }: { id: string; load: (signal: AbortSignal) => Promise<string[]> }) {
  const query = useSharedAdminOptionQuery({ cacheKey: 'assets', empty: [], enabled: true, load, staleTime: 60_000 });
  return <div data-testid={id}>{query.loading ? '加载中' : query.data.join(',')}</div>;
}

function renderConsumers(children: ReactNode) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}>{children}</QueryClientProvider>);
}

describe('useSharedAdminOptionQuery', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    authStore.setSession({
      accessToken: 'access-a',
      generation: `generation-${crypto.randomUUID()}`,
      refreshToken: 'refresh-a',
      scope: 'admin',
      subject: 'admin:a'
    });
  });

  it('同一会话与 cacheKey 的多个消费者只发起一次请求', async () => {
    const result = deferred<string[]>();
    const load = vi.fn(() => result.promise);
    renderConsumers(
      <>
        <Consumer id="first" load={load} />
        <Consumer id="second" load={load} />
      </>
    );

    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
    act(() => result.resolve(['BTC', 'ETH']));

    await waitFor(() => expect(screen.getByTestId('first')).toHaveTextContent('BTC,ETH'));
    expect(screen.getByTestId('second')).toHaveTextContent('BTC,ETH');
  });

  it('按引用计数保留共享请求，最后一个消费者卸载时 Abort', async () => {
    const result = deferred<string[]>();
    let requestSignal: AbortSignal | undefined;
    const load = vi.fn((signal: AbortSignal) => {
      requestSignal = signal;
      return result.promise;
    });
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const view = (children: ReactNode) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
    const { rerender, unmount } = render(view(
      <>
        <Consumer id="first" load={load} />
        <Consumer id="second" load={load} />
      </>
    ));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));

    rerender(view(<Consumer id="first" load={load} />));
    expect(requestSignal?.aborted).toBe(false);
    unmount();
    expect(requestSignal?.aborted).toBe(true);
  });

  it('主体或会话代数变化时终止旧查询，且新会话不复用旧缓存', async () => {
    const first = deferred<string[]>();
    const second = deferred<string[]>();
    const signals: AbortSignal[] = [];
    const load = vi.fn((signal: AbortSignal) => {
      signals.push(signal);
      return signals.length === 1 ? first.promise : second.promise;
    });
    renderConsumers(<Consumer id="value" load={load} />);
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));

    act(() => {
      authStore.setSession({
        accessToken: 'access-b',
        generation: 'generation-b',
        refreshToken: 'refresh-b',
        scope: 'admin',
        subject: 'admin:b'
      });
    });

    await waitFor(() => expect(signals[0].aborted).toBe(true));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    act(() => {
      first.resolve(['A-only']);
      second.resolve(['B-only']);
    });
    await waitFor(() => expect(screen.getByTestId('value')).toHaveTextContent('B-only'));
    expect(screen.getByTestId('value')).not.toHaveTextContent('A-only');
  });
});
