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

  it('returns undefined for 204 responses', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(apiRequest('/admin/api/v1/test')).resolves.toBeUndefined();
  });

  it('throws ApiError with backend payload and clears session on 401', async () => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'unauthorized' }), {
          status: 401,
          statusText: 'Unauthorized'
        })
      )
    );

    await expect(apiRequest('/admin/api/v1/test')).rejects.toMatchObject({
      status: 401,
      code: 'UNAUTHORIZED',
      message: 'unauthorized'
    });
    expect(authStore.getSession()).toBeNull();
  });
});
