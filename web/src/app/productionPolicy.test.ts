import { describe, expect, it } from 'vitest';

import { adminPermissionForRequest } from '../admin/access';
import { resolveBackendRuntimeConfig } from '../config/backend';
import { createAppQueryClient } from './providers';

describe('Admin 生产策略', () => {
  it('写请求默认不重放，读查询仅有限重试', () => {
    const options = createAppQueryClient().getDefaultOptions();
    expect(options.mutations?.retry).toBe(false);
    expect(options.queries?.retry).toBe(1);
    expect(options.queries?.refetchOnWindowFocus).toBe(false);
  });

  it('未登记 Admin API 动作失败关闭到 admin.unmapped', () => {
    expect(adminPermissionForRequest('/admin/api/v1/new-business/action', 'POST')).toBe('admin.unmapped.write');
    expect(adminPermissionForRequest('/admin/api/v1/new-business/action', 'GET')).toBe('admin.unmapped.read');
  });

  it('生产 API 配置要求显式模式且非同源时必须 HTTPS', () => {
    expect(() =>
      resolveBackendRuntimeConfig(
        { VITE_API_BASE_URL: 'http://api.example.test', VITE_API_SAME_ORIGIN: 'false' },
        { production: true }
      )
    ).toThrow();
    expect(resolveBackendRuntimeConfig({ VITE_API_SAME_ORIGIN: 'true' }, { production: true }).mode).toBe('same-origin');
  });
});
