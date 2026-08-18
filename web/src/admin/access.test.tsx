import { describe, expect, it } from 'vitest';

import {
  adminMutationPermissionsForEndpoint,
  adminReadPermissionForPath,
  hasAdminPermission,
  type AdminAccess
} from './access';

function access(permissions: string[]): AdminAccess {
  return {
    admin_id: 1,
    username: 'admin',
    role_id: 2,
    role_name: '测试角色',
    permissions,
    is_super_admin: permissions.includes('*')
  };
}

describe('admin access mapping', () => {
  it('supports exact, domain wildcard and global wildcard permissions', () => {
    expect(hasAdminPermission(access(['wallet.assets.read']), 'wallet.assets.read')).toBe(true);
    expect(hasAdminPermission(access(['wallet.*']), 'wallet.assets.write')).toBe(true);
    expect(hasAdminPermission(access(['*']), 'admin.unmapped.write')).toBe(true);
    expect(hasAdminPermission(access(['wallet.assets.read']), 'wallet.assets.write')).toBe(false);
  });

  it('maps frontend routes to the same stable backend resources', () => {
    expect(adminReadPermissionForPath('/admin/market/feed-config')).toBe('market.feed.read');
    expect(adminReadPermissionForPath('/admin/wallet/deposit-address-pool')).toBe('wallet.address_pool.read');
    expect(adminReadPermissionForPath('/admin/system/brand')).toBe('system.brand.read');
    expect(adminReadPermissionForPath('/admin/users/kyc/settings')).toBe('users.kyc.read');
    expect(adminReadPermissionForPath('/admin/users/kyc/reviews')).toBe('users.kyc.read');
    expect(adminReadPermissionForPath('/admin/prediction/sync')).toBe('prediction.sync.read');
    expect(adminReadPermissionForPath('/admin/prediction/assets')).toBe('prediction.settings.read');
    expect(adminReadPermissionForPath('/admin/account/security')).toBe('account.security.read');
    expect(adminReadPermissionForPath('/admin/config-center')).toBe('config_center.read');
    expect(adminReadPermissionForPath('/admin/not-mapped')).toBe('admin.unmapped.read');
  });

  it('maps resource endpoints and fails closed for unknown endpoints', () => {
    expect(adminMutationPermissionsForEndpoint('/admin/api/v1/market-pairs')).toContain('market.pairs.write');
    expect(adminMutationPermissionsForEndpoint('/admin/api/v1/new-coins')).toContain('new_coin.projects.write');
    expect(adminMutationPermissionsForEndpoint('/admin/api/v1/config-center')).toContain('config_center.write');
    expect(adminMutationPermissionsForEndpoint('/admin/api/v1/not-mapped')).toContain('admin.unmapped.write');
  });
});
