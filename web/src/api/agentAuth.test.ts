import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
import { agentLogin } from './agentAuth';

describe('agentLogin', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it('uses the agent session scope without clearing admin session on 401', async () => {
    authStore.setSession({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:1' });
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'unauthorized' }), {
        status: 401,
        statusText: 'Unauthorized'
      })
    );
    vi.stubGlobal('fetch', fetchMock);

    await expect(agentLogin({ username: 'agent', password: 'bad-password' })).rejects.toMatchObject({
      status: 401,
      code: 'UNAUTHORIZED'
    });

    const headers = fetchMock.mock.calls[0][1].headers as Headers;
    expect(headers.get('Authorization')).toBe('Bearer agent-token');
    expect(authStore.getSession()).toEqual({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    expect(authStore.getSession('agent')).toBeNull();
  });
});
