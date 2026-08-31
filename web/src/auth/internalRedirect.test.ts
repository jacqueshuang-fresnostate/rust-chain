import { describe, expect, it } from 'vitest';

import { safeInternalRedirect } from './internalRedirect';

describe('safeInternalRedirect', () => {
  it('保留合法站内 path/search/hash', () => {
    expect(safeInternalRedirect('/admin/users?status=disabled#user-7', '/admin/dashboard', '/admin')).toBe(
      '/admin/users?status=disabled#user-7'
    );
  });

  it('拒绝外部、协议相对、反斜线、控制字符和跨壳路径', () => {
    const fallback = '/admin/dashboard';
    [
      'https://evil.example/admin',
      '//evil.example/admin',
      '/\\evil.example/admin',
      '/admin/users\nnext',
      '/agent/dashboard'
    ].forEach((value) => {
      expect(safeInternalRedirect(value, fallback, '/admin')).toBe(fallback);
    });
  });
});
