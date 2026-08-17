import { afterEach, describe, expect, it, vi } from 'vitest';

import type { TurnstileApi } from './turnstile';

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

describe('Turnstile lifecycle utility', () => {
  afterEach(() => {
    delete window.turnstile;
    document.querySelectorAll('script[src*="challenges.cloudflare.com/turnstile"]').forEach((script) => script.remove());
  });

  it('reuses one module loader, waits for ready, and retries after script failure', async () => {
    vi.resetModules();
    const { loadTurnstileApi } = await import('./turnstile');
    const firstLoad = loadTurnstileApi();
    const sharedLoad = loadTurnstileApi();
    expect(firstLoad).toBe(sharedLoad);
    expect(document.querySelectorAll('script[src*="challenges.cloudflare.com/turnstile"]')).toHaveLength(1);
    const firstScript = document.querySelector<HTMLScriptElement>('script[src*="challenges.cloudflare.com/turnstile"]');
    expect(firstScript?.async).toBe(false);
    expect(firstScript?.defer).toBe(false);

    const failedLoads = Promise.allSettled([firstLoad, sharedLoad]);
    document.querySelector<HTMLScriptElement>('script[src*="challenges.cloudflare.com/turnstile"]')
      ?.dispatchEvent(new Event('error'));
    expect((await failedLoads).map((result) => result.status)).toEqual(['rejected', 'rejected']);
    expect(document.querySelectorAll('script[src*="challenges.cloudflare.com/turnstile"]')).toHaveLength(0);

    const retryLoad = loadTurnstileApi();
    expect(retryLoad).not.toBe(firstLoad);
    const retryScript = document.querySelector<HTMLScriptElement>('script[src*="challenges.cloudflare.com/turnstile"]');
    expect(retryScript?.async).toBe(false);
    expect(retryScript?.defer).toBe(false);
    const ready = deferred<void>();
    const readyMock = vi.fn((callback: () => void) => {
      void ready.promise.then(callback);
    });
    const api: TurnstileApi = {
      ready: readyMock,
      render: vi.fn(() => 'widget'),
      reset: vi.fn(),
      remove: vi.fn(),
    };
    window.turnstile = api;
    document.querySelector<HTMLScriptElement>('script[src*="challenges.cloudflare.com/turnstile"]')
      ?.dispatchEvent(new Event('load'));
    await Promise.resolve();
    expect(readyMock).toHaveBeenCalledTimes(1);

    let resolved = false;
    void retryLoad.then(() => {
      resolved = true;
    });
    await Promise.resolve();
    expect(resolved).toBe(false);
    ready.resolve();
    await expect(retryLoad).resolves.toBe(api);
  });

  it('reuses a completed async/defer script without calling ready', async () => {
    vi.resetModules();
    const script = document.createElement('script');
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
    script.async = true;
    script.defer = true;
    script.dataset.turnstileLoaderState = 'loaded';
    document.head.appendChild(script);

    const readyMock = vi.fn(() => {
      throw new Error('ready must not run for an already loaded async/defer script');
    });
    const api: TurnstileApi = {
      ready: readyMock,
      render: vi.fn(() => 'widget'),
      reset: vi.fn(),
      remove: vi.fn(),
    };
    window.turnstile = api;
    const { loadTurnstileApi } = await import('./turnstile');

    await expect(loadTurnstileApi()).resolves.toBe(api);
    expect(readyMock).not.toHaveBeenCalled();
    expect(document.querySelectorAll('script[src*="challenges.cloudflare.com/turnstile"]')).toHaveLength(1);
    expect(script.async).toBe(true);
    expect(script.defer).toBe(true);
  });

  it('cancels slow or disconnected generations before render', async () => {
    vi.resetModules();
    const { createTurnstileLifecycle } = await import('./turnstile');
    const apiReady = deferred<TurnstileApi>();
    const loadStarted = deferred<void>();
    const renderMock = vi.fn(() => 'widget');
    const api: TurnstileApi = {
      ready: (callback) => callback(),
      render: renderMock,
      reset: vi.fn(),
      remove: vi.fn(),
    };
    const lifecycle = createTurnstileLifecycle({
      loadApi: () => {
        loadStarted.resolve();
        return apiReady.promise;
      },
    });
    const container = document.createElement('div');
    document.body.appendChild(container);
    const pendingRender = lifecycle.render({
      resolveContainer: () => container,
      isContainerCurrent: (candidate) => candidate === container,
      options: { sitekey: 'admin-site-key' },
    });

    await loadStarted.promise;
    lifecycle.remove();
    apiReady.resolve(api);
    await expect(pendingRender).resolves.toBeNull();
    expect(renderMock).not.toHaveBeenCalled();

    const disconnectedApi = deferred<TurnstileApi>();
    const disconnectedLifecycle = createTurnstileLifecycle({ loadApi: () => disconnectedApi.promise });
    const disconnectedRender = disconnectedLifecycle.render({
      resolveContainer: () => container,
      isContainerCurrent: (candidate) => candidate === container,
      options: { sitekey: 'admin-site-key' },
    });
    await Promise.resolve();
    container.remove();
    disconnectedApi.resolve(api);
    await expect(disconnectedRender).resolves.toBeNull();
    expect(renderMock).not.toHaveBeenCalled();
  });
});
