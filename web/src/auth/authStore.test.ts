import { beforeEach, describe, expect, it } from 'vitest';

import { authStore, SESSION_STORAGE_KEY, type AuthSession } from './authStore';

const adminSession: AuthSession = {
  accessToken: 'access',
  refreshToken: 'refresh',
  scope: 'admin',
  subject: 'admin:1'
};

describe('authStore', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('saves and restores an admin session from the exchange admin key', () => {
    authStore.setSession(adminSession);

    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(authStore.getSession()).toEqual(adminSession);
  });

  it('rejects malformed stored session values safely', () => {
    localStorage.setItem(SESSION_STORAGE_KEY, '{bad json');
    expect(authStore.getSession()).toBeNull();

    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, scope: 'guest' }));
    expect(authStore.getSession()).toBeNull();

    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, accessToken: '' }));
    expect(authStore.getSession()).toBeNull();
  });

  it('clears the stored session', () => {
    authStore.setSession(adminSession);
    authStore.clearSession();

    expect(authStore.getSession()).toBeNull();
    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });
});
