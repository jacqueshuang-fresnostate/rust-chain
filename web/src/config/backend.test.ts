import { describe, expect, it } from 'vitest';

import { buildApiUrl, buildWebSocketUrl, resolveBackendRuntimeConfig } from './backend';

describe('后端运行时配置', () => {
  it('要求显式选择同源或独立后端模式', () => {
    expect(() => resolveBackendRuntimeConfig({ VITE_API_BASE_URL: 'https://api.example.test' })).toThrow(
      'VITE_API_SAME_ORIGIN 必须显式设置为 true 或 false'
    );
    expect(() => resolveBackendRuntimeConfig({ VITE_API_SAME_ORIGIN: 'false' })).toThrow(
      '非同源模式必须设置 VITE_API_BASE_URL'
    );
    expect(() =>
      resolveBackendRuntimeConfig({ VITE_API_BASE_URL: 'https://api.example.test', VITE_API_SAME_ORIGIN: 'true' })
    ).toThrow('同源模式不得同时设置 VITE_API_BASE_URL');
  });

  it('在同源模式使用相对 REST 地址与同源 WebSocket 地址', () => {
    expect(resolveBackendRuntimeConfig({ VITE_API_SAME_ORIGIN: 'true' })).toEqual({ apiBaseUrl: '', mode: 'same-origin' });
  });

  it('拒绝非 HTTP 协议、携带路径和生产非 HTTPS 的独立后端', () => {
    expect(() =>
      resolveBackendRuntimeConfig({ VITE_API_BASE_URL: 'file:///tmp/api', VITE_API_SAME_ORIGIN: 'false' })
    ).toThrow('仅支持 HTTP(S)');
    expect(() =>
      resolveBackendRuntimeConfig({ VITE_API_BASE_URL: 'https://api.example.test/v1', VITE_API_SAME_ORIGIN: 'false' })
    ).toThrow('必须是纯 Origin');
    expect(() =>
      resolveBackendRuntimeConfig(
        { VITE_API_BASE_URL: 'http://api.example.test', VITE_API_SAME_ORIGIN: 'false' },
        { production: true }
      )
    ).toThrow('生产环境');
  });

  it('当前测试环境的 REST 与 WebSocket 使用同一 API origin', () => {
    expect(buildApiUrl('/admin/api/v1/access/me')).toBe('http://127.0.0.1:8080/admin/api/v1/access/me');
    expect(buildWebSocketUrl('/ws/public')).toBe('ws://127.0.0.1:8080/ws/public');
  });
});
