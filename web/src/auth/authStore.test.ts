import { beforeEach, describe, expect, it } from 'vitest';

import { AGENT_SESSION_STORAGE_KEY, authStore, SESSION_STORAGE_KEY, type AuthSession } from './authStore';

const adminSession: AuthSession = {
  accessToken: 'access',
  refreshToken: 'refresh',
  scope: 'admin',
  subject: 'admin:1'
};

const agentSession: AuthSession = {
  accessToken: 'agent-access',
  refreshToken: 'agent-refresh',
  scope: 'agent',
  subject: 'agent:1'
};

describe('authStore', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('saves and restores admin and agent sessions from separate keys', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);

    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(localStorage.getItem(AGENT_SESSION_STORAGE_KEY)).toBe(JSON.stringify(agentSession));
    expect(authStore.getSession()).toEqual(adminSession);
    expect(authStore.getSession('admin')).toEqual(adminSession);
    expect(authStore.getSession('agent')).toEqual(agentSession);
  });

  it('rejects malformed stored session values safely', () => {
    localStorage.setItem(SESSION_STORAGE_KEY, '{bad json');
    expect(authStore.getSession()).toBeNull();

    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, scope: 'guest' }));
    expect(authStore.getSession()).toBeNull();

    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, accessToken: '' }));
    expect(authStore.getSession()).toBeNull();
  });

  it('clears only the requested scope session', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);
    authStore.clearSession('agent');

    expect(authStore.getSession()).toEqual(adminSession);
    expect(authStore.getSession('agent')).toBeNull();
    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(localStorage.getItem(AGENT_SESSION_STORAGE_KEY)).toBeNull();
  });

  it('defaults clearSession to admin scope', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);
    authStore.clearSession();

    expect(authStore.getSession()).toBeNull();
    expect(authStore.getSession('agent')).toEqual(agentSession);
  });
});
