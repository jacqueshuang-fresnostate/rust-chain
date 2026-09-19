import { cleanup, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  AdminAccessProvider, AdminRequestActionBoundary, adminActionForRequest,
  adminPermissionForEndpoint, adminPermissionForRequest, hasAdminPermission,
  type AdminAccess, type AdminHttpMethod
} from './access';

const methods: AdminHttpMethod[] = ['GET', 'POST', 'HEAD', 'OPTIONS', 'PATCH', 'PUT', 'DELETE'];
const root = '/financial-reconciliation/snapshots';
const paths = [root, `${root}/1`, `${root}/18446744073709551615`, `${root}/1/follow-ups`, `${root}/18446744073709551615/follow-ups`];
const writable = (path: string) => path === root || path.endsWith('/follow-ups');
const unmapped = (method: AdminHttpMethod) => `admin.unmapped.${['GET', 'HEAD', 'OPTIONS'].includes(method) ? 'read' : 'write'}`;
const access = (action: string): AdminAccess => ({
  admin_id: 1, username: 'snapshot-test', role_id: 1, role_name: '测试角色',
  permissions: [`governance.financial.${action}`], is_super_admin: false
});

describe('资金对账快照精确权限映射', () => {
  it.each(paths)('集合、详情、跟进仅开放指定方法 %s', (path) => {
    for (const prefix of ['', '/admin/api/v1']) {
      for (const query of ['', '?limit=20&offset=0']) {
        const endpoint = `${prefix}${path}${query}`;
        for (const method of methods) {
          const expected = method === 'GET' ? 'governance.financial.read'
            : method === 'POST' && writable(path) ? 'governance.financial.operate' : unmapped(method);
          expect(adminPermissionForRequest(endpoint, method), `${method} ${endpoint}`).toBe(expected);
        }
        if (writable(path)) {
          expect(adminActionForRequest(endpoint, 'POST')).toBe('operate');
          expect(adminPermissionForEndpoint(endpoint, 'operate')).toBe('governance.financial.operate');
        }
      }
    }
  });

  it('畸形 ID、越界、编码和多余路径段全部失败关闭', () => {
    const invalidPaths = [
      '/financial-reconciliation-export', `${root}-export`, `${root}/`,
      '/financial-reconciliation//snapshots', `${root}/1/`, `${root}/1/extra`,
      `${root}/1/follow-ups/`, `${root}/1/follow-ups/2`, `${root}/1//follow-ups`,
      `${root}/1/follow-ups-export`, `${root}/1/follow-ups\n`
    ];
    for (const id of ['', '0', '00', '01', '-1', '+1', '1.0', '1e2', ' 1', '1 ', '1\n', '1\r', '%31', '%2F1', '١', 'not-an-id', '18446744073709551616']) {
      invalidPaths.push(`${root}/${id}`, `${root}/${id}/follow-ups`);
    }
    for (const path of invalidPaths) {
      for (const method of methods) {
        expect(adminPermissionForRequest(`/admin/api/v1${path}`, method), `${method} ${path}`).toBe(unmapped(method));
      }
      expect(adminPermissionForEndpoint(path, 'operate')).toBe('admin.unmapped.operate');
    }
  });

  it('保留实时报表根路径的既有只读方法，不开放采集写入', () => {
    for (const method of methods) {
      expect(adminPermissionForRequest('/admin/api/v1/financial-reconciliation', method))
        .toBe(['GET', 'HEAD', 'OPTIONS'].includes(method) ? 'governance.financial.read' : 'admin.unmapped.write');
    }
  });

  it('read、write、review、settle 均不能替代 operate，也不新增默认授权', () => {
    for (const action of ['read', 'write', 'review', 'operate', 'settle']) {
      for (const path of paths) {
        expect(hasAdminPermission(access(action), adminPermissionForRequest(path, 'GET')!)).toBe(action === 'read');
        expect(hasAdminPermission(access(action), adminPermissionForRequest(path, 'POST')!)).toBe(action === 'operate' && writable(path));
        expect(hasAdminPermission(access(action), adminPermissionForRequest(path, 'PATCH')!)).toBe(false);
      }
    }
    expect(hasAdminPermission({ ...access('read'), permissions: [] }, 'governance.financial.operate')).toBe(false);
  });

  it('真实 UI 动作门隔离读取、采集与跟进，拒绝详情 POST', () => {
    for (const action of ['read', 'operate', 'write']) {
      render(
        <AdminAccessProvider access={access(action)}>
          <AdminRequestActionBoundary endpoint={root} method="GET"><span>读取快照</span></AdminRequestActionBoundary>
          <AdminRequestActionBoundary endpoint={root} method="POST"><span>采集快照</span></AdminRequestActionBoundary>
          <AdminRequestActionBoundary endpoint={`${root}/1/follow-ups`} method="POST"><span>登记跟进</span></AdminRequestActionBoundary>
          <AdminRequestActionBoundary endpoint={`${root}/1`} method="POST"><span>未登记动作</span></AdminRequestActionBoundary>
        </AdminAccessProvider>
      );
      expect(Boolean(screen.queryByText('读取快照'))).toBe(action === 'read');
      expect(Boolean(screen.queryByText('采集快照'))).toBe(action === 'operate');
      expect(Boolean(screen.queryByText('登记跟进'))).toBe(action === 'operate');
      expect(screen.queryByText('未登记动作')).not.toBeInTheDocument();
      cleanup();
    }
  });
});
