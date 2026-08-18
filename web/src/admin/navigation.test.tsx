import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';

import { adminNavItems } from './navigation';
import { adminRoutes } from './routes';

const navPaths = adminNavItems
  .flatMap((item) => [item.path, ...(item.children?.map((child) => child.path) ?? [])])
  .filter((path): path is string => Boolean(path));

const routePaths = new Set(
  adminRoutes.flatMap((route) => (typeof route.path === 'string' ? [`/admin/${route.path}`] : []))
);

describe('admin navigation registry', () => {
  it('only references registered admin routes', () => {
    const unknown = navPaths.filter((path) => !routePaths.has(path));
    expect(unknown).toEqual([]);
  });

  it('never lists the same path twice', () => {
    expect(new Set(navPaths).size).toBe(navPaths.length);
  });

  it('gives every nav group a distinct icon', () => {
    const iconTypes = adminNavItems.map((item) => (item.icon as ReactElement | undefined)?.type);
    expect(iconTypes.every(Boolean)).toBe(true);
    expect(new Set(iconTypes).size).toBe(iconTypes.length);
  });

  it('separates configuration, operations, and the current administrator account', () => {
    expect(adminNavItems).toContainEqual(
      expect.objectContaining({ label: '配置中心', path: '/admin/config-center' })
    );
    const users = adminNavItems.find((item) => item.label === '用户与代理');
    const prediction = adminNavItems.find((item) => item.label === '竞猜管理');
    const system = adminNavItems.find((item) => item.label === '系统配置');
    const account = adminNavItems.find((item) => item.label === '我的账号');

    expect(users?.children).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ label: 'KYC 规则配置', path: '/admin/users/kyc/settings' }),
        expect.objectContaining({ label: 'KYC 审核队列', path: '/admin/users/kyc/reviews' })
      ])
    );
    expect(prediction?.children).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ label: '竞猜配置', path: '/admin/prediction/settings' }),
        expect.objectContaining({ label: '同步运行', path: '/admin/prediction/sync' })
      ])
    );
    expect(system?.children?.some((item) => item.path === '/admin/system/two-factor')).toBe(false);
    expect(account?.children).toEqual([
      expect.objectContaining({ label: '账号安全', path: '/admin/account/security' })
    ]);
    expect(navPaths).not.toContain('/admin/users/kyc');
    expect(navPaths).not.toContain('/admin/prediction/assets');
    expect(navPaths).not.toContain('/admin/prediction/sync-logs');
  });
});
