import { afterEach, describe, expect, it, vi } from 'vitest';

describe('Admin 同源生产配置', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  it('编译为相对 REST 地址并从页面 Origin 派生 WebSocket 地址', async () => {
    vi.stubEnv('VITE_API_SAME_ORIGIN', 'true');
    vi.stubEnv('VITE_API_BASE_URL', '');
    vi.resetModules();

    const { backendRuntimeConfig, buildApiUrl, buildWebSocketUrl } = await import('./backend');

    expect(backendRuntimeConfig).toEqual({ apiBaseUrl: '', mode: 'same-origin' });
    expect(buildApiUrl('/admin/api/v1/access/me')).toBe('/admin/api/v1/access/me');
    expect(buildWebSocketUrl('/ws/public', { href: 'https://admin.example.test/settings?tab=security' })).toBe(
      'wss://admin.example.test/ws/public'
    );
    expect(buildWebSocketUrl('/ws/public', { href: 'http://127.0.0.1:8080/admin' })).toBe(
      'ws://127.0.0.1:8080/ws/public'
    );
  });
});
