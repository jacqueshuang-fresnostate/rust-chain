import { beforeEach, describe, expect, it, vi } from 'vitest';

import { getLoginConfig } from './adminAuth';

describe('getLoginConfig', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.unstubAllGlobals();
  });

  it('loads the shared public login policy before the admin path', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          username_login_enabled: false,
          cf_turnstile_enabled: true,
          cf_turnstile_site_key: 'site-key',
        }),
        { status: 200 },
      ),
    );
    vi.stubGlobal('fetch', fetchMock);

    await expect(getLoginConfig()).resolves.toEqual({
      usernameLoginEnabled: false,
      cfTurnstileEnabled: true,
      cfTurnstileSiteKey: 'site-key',
    });
    expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/api\/v1\/auth\/login\/config$/);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('falls back to the admin login policy endpoint when the public path is unavailable', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ code: 'HTTP_503', message: 'unavailable' }), {
          status: 503,
          statusText: 'Service Unavailable',
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            username_login_enabled: true,
            cf_turnstile_enabled: true,
            cf_turnstile_site_key: 'admin-site-key',
          }),
          { status: 200 },
        ),
      );
    vi.stubGlobal('fetch', fetchMock);

    await expect(getLoginConfig()).resolves.toEqual({
      usernameLoginEnabled: true,
      cfTurnstileEnabled: true,
      cfTurnstileSiteKey: 'admin-site-key',
    });
    expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/api\/v1\/auth\/login\/config$/);
    expect(String(fetchMock.mock.calls[1][0])).toMatch(/\/admin\/api\/v1\/auth\/login\/config$/);
  });
});
