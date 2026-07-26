import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
import { apiRequest } from './client';

describe('apiRequest', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('adds JSON headers and bearer token, then returns JSON', async () => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:1' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    const result = await apiRequest<{ ok: boolean }>('/admin/api/v1/test', { method: 'POST', body: JSON.stringify({ id: 1 }) });

    expect(result).toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:8080/admin/api/v1/test',
      expect.objectContaining({
        method: 'POST',
        headers: expect.any(Headers)
      })
    );
    const headers = fetchMock.mock.calls[0][1].headers as Headers;
    expect(headers.get('Content-Type')).toBe('application/json');
    expect(headers.get('Authorization')).toBe('Bearer token');
  });

  it('does not set JSON content type for FormData while keeping bearer token', async () => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    const formData = new FormData();
    formData.append('file', new File(['GIF89a'], 'image.gif', { type: 'image/gif' }));

    const result = await apiRequest<{ ok: boolean }>('/admin/api/v1/uploads/images', { method: 'POST', body: formData });

    expect(result).toEqual({ ok: true });
    const headers = fetchMock.mock.calls[0][1].headers as Headers;
    expect(headers.has('Content-Type')).toBe(false);
    expect(headers.get('Authorization')).toBe('Bearer token');
  });

  it('uses the requested auth scope token', async () => {
    authStore.setSession({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:1' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    await apiRequest('/agent/api/v1/me', { authScope: 'agent' });

    const headers = fetchMock.mock.calls[0][1].headers as Headers;
    expect(headers.get('Authorization')).toBe('Bearer agent-token');
  });

  it('returns undefined for 204 responses', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(apiRequest('/admin/api/v1/test')).resolves.toBeUndefined();
  });

  it('refreshes the session once on 401 and replays the original request', async () => {
    authStore.setSession({ accessToken: 'expired', refreshToken: 'refresh-1', scope: 'admin', subject: 'admin:1' });
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'expired' }), { status: 401 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: 'fresh', refresh_token: 'refresh-2', token_type: 'Bearer', scope: 'admin' }), { status: 200 })
      )
      .mockResolvedValueOnce(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    const result = await apiRequest<{ ok: boolean }>('/admin/api/v1/test');

    expect(result).toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(fetchMock.mock.calls[1][0]).toBe('http://127.0.0.1:8080/admin/api/v1/auth/refresh');
    expect(JSON.parse(String(fetchMock.mock.calls[1][1].body))).toEqual({ refresh_token: 'refresh-1' });
    const replayHeaders = fetchMock.mock.calls[2][1].headers as Headers;
    expect(replayHeaders.get('Authorization')).toBe('Bearer fresh');
    expect(authStore.getSession()).toMatchObject({ accessToken: 'fresh', refreshToken: 'refresh-2' });
  });

  it('shares one in-flight refresh across concurrent 401 responses', async () => {
    authStore.setSession({ accessToken: 'expired', refreshToken: 'refresh-1', scope: 'admin', subject: 'admin:1' });
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith('/admin/api/v1/auth/refresh')) {
        return new Response(JSON.stringify({ access_token: 'fresh', refresh_token: 'refresh-2', token_type: 'Bearer', scope: 'admin' }), { status: 200 });
      }
      return new Headers(init?.headers).get('Authorization') === 'Bearer fresh'
        ? new Response(JSON.stringify({ ok: true }), { status: 200 })
        : new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'expired' }), { status: 401 });
    });
    vi.stubGlobal('fetch', fetchMock);

    await Promise.all([apiRequest('/admin/api/v1/a'), apiRequest('/admin/api/v1/b')]);

    expect(fetchMock.mock.calls.filter(([input]) => String(input).endsWith('/admin/api/v1/auth/refresh'))).toHaveLength(1);
    expect(authStore.getSession()).toMatchObject({ accessToken: 'fresh', refreshToken: 'refresh-2' });
  });

  it('throws ApiError with backend payload and clears only the failed scope session on 401', async () => {
    authStore.setSession({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:1' });
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'unauthorized' }), {
          status: 401,
          statusText: 'Unauthorized'
        })
      )
    );

    await expect(apiRequest('/agent/api/v1/me', { authScope: 'agent' })).rejects.toMatchObject({
      status: 401,
      code: 'UNAUTHORIZED',
      message: 'unauthorized'
    });
    expect(authStore.getSession()).toEqual({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    expect(authStore.getSession('agent')).toBeNull();
  });
});
