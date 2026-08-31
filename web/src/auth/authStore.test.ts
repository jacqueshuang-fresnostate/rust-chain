import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  AGENT_SESSION_STORAGE_KEY,
  AUTH_SYNC_STORAGE_KEY,
  authSubjectFromAccessToken,
  authStore,
  SESSION_STORAGE_KEY,
  type AuthSession
} from './authStore';

const adminSession: AuthSession = {
  accessToken: 'access',
  generation: 'admin-generation',
  refreshToken: 'refresh',
  scope: 'admin',
  subject: 'admin:1'
};

const agentSession: AuthSession = {
  accessToken: 'agent-access',
  generation: 'agent-generation',
  refreshToken: 'agent-refresh',
  scope: 'agent',
  subject: 'agent:1'
};

describe('authStore', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });

  it('saves and restores admin and agent sessions from separate keys', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);

    expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(sessionStorage.getItem(AGENT_SESSION_STORAGE_KEY)).toBe(JSON.stringify(agentSession));
    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
    expect(authStore.getSession()).toEqual(adminSession);
    expect(authStore.getSession('admin')).toEqual(adminSession);
    expect(authStore.getSession('agent')).toEqual(agentSession);
  });

  it('rejects malformed stored session values safely', () => {
    sessionStorage.setItem(SESSION_STORAGE_KEY, '{bad json');
    expect(authStore.getSession()).toBeNull();

    sessionStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, scope: 'guest' }));
    expect(authStore.getSession()).toBeNull();

    sessionStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, accessToken: '' }));
    expect(authStore.getSession()).toBeNull();
  });

  it('clears only the requested scope session', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);
    authStore.clearSession('agent');

    expect(authStore.getSession()).toEqual(adminSession);
    expect(authStore.getSession('agent')).toBeNull();
    expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(sessionStorage.getItem(AGENT_SESSION_STORAGE_KEY)).toBeNull();
  });

  it('defaults clearSession to admin scope', () => {
    authStore.setSession(adminSession);
    authStore.setSession(agentSession);
    authStore.clearSession();

    expect(authStore.getSession()).toBeNull();
    expect(authStore.getSession('agent')).toEqual(agentSession);
  });

  it('仅一次将旧 localStorage 会话迁移到 sessionStorage', () => {
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(adminSession));

    expect(authStore.getSession('admin')).toEqual(adminSession);
    expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBe(JSON.stringify(adminSession));
    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });

  it('迁移时丢弃非法旧会话，不将坏数据复制到 sessionStorage', () => {
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...adminSession, accessToken: '' }));

    expect(authStore.getSession('admin')).toBeNull();
    expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
    expect(localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });

  it('使用 generation 和 refresh token 双重 CAS，旧刷新结果不得覆盖新会话', () => {
    authStore.setSession(adminSession);
    expect(
      authStore.compareAndSetSession('admin', 'wrong-generation', adminSession.refreshToken, {
        accessToken: 'stale-access',
        refreshToken: 'stale-refresh'
      })
    ).toBe(false);
    expect(
      authStore.compareAndSetSession('admin', adminSession.generation, 'wrong-refresh', {
        accessToken: 'stale-access',
        refreshToken: 'stale-refresh'
      })
    ).toBe(false);
    expect(authStore.getSession('admin')).toEqual(adminSession);

    expect(
      authStore.compareAndSetSession('admin', adminSession.generation, adminSession.refreshToken, {
        accessToken: 'fresh-access',
        refreshToken: 'fresh-refresh'
      })
    ).toBe(true);
    expect(authStore.getSession('admin')).toMatchObject({
      accessToken: 'fresh-access',
      generation: adminSession.generation,
      refreshToken: 'fresh-refresh'
    });
  });

  it('接收其他标签页的退出或会话替换信号后清理当前标签令牌', () => {
    authStore.setSession(adminSession);
    const listener = vi.fn();
    const unsubscribe = authStore.subscribe(listener);

    window.dispatchEvent(
      new StorageEvent('storage', {
        key: AUTH_SYNC_STORAGE_KEY,
        newValue: JSON.stringify({ at: Date.now(), scope: 'admin', sourceId: 'another-tab', type: 'cleared' })
      })
    );

    expect(authStore.getSession('admin')).toBeNull();
    expect(sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
    expect(listener).toHaveBeenCalled();
    unsubscribe();
  });

  it('跨标签信号仅携带无敏感元数据', () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem');
    authStore.setSession(adminSession);

    const signal = setItem.mock.calls.find(([key]) => key === AUTH_SYNC_STORAGE_KEY)?.[1];
    expect(signal).toBeTruthy();
    expect(signal).not.toContain(adminSession.accessToken);
    expect(signal).not.toContain(adminSession.refreshToken);
  });

  it('从旧登录 JWT 提取与 scope 匹配的 subject，不将 payload 当授权证据', () => {
    const encode = (value: unknown) =>
      btoa(JSON.stringify(value)).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
    const token = `${encode({ alg: 'none' })}.${encode({ sub: 'admin:77' })}.signature`;

    expect(authSubjectFromAccessToken(token, 'admin')).toBe('admin:77');
    expect(authSubjectFromAccessToken(token, 'agent')).toBe('agent:unknown');
    expect(authSubjectFromAccessToken('opaque-token', 'admin')).toBe('admin:unknown');
  });
});
