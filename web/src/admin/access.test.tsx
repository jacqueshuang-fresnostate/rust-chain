import { cleanup, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  adminActionForRequest,
  AdminAccessProvider,
  AdminRequestActionBoundary,
  adminPermissionForRequest,
  adminReadPermissionForPath,
  hasAdminPermission,
  type AdminHttpMethod,
  type AdminAccess
} from './access';

type ActionDescriptor = {
  endpoint: string;
  method: AdminHttpMethod;
  permission: string | null;
};

const actionDescriptorMatrix: ActionDescriptor[] = [
  { endpoint: '/admin/api/v1/agents/1/password/reset', method: 'POST', permission: 'agents.write' },
  { endpoint: '/admin/api/v1/users/1/agent', method: 'PATCH', permission: 'users.write' },
  { endpoint: '/admin/api/v1/agents/1/status', method: 'PATCH', permission: 'agents.write' },
  { endpoint: '/admin/api/v1/agents', method: 'POST', permission: 'agents.write' },
  { endpoint: '/admin/api/v1/kyc/config', method: 'PATCH', permission: 'users.kyc.write' },
  { endpoint: '/admin/api/v1/kyc/submissions/1/review', method: 'PATCH', permission: 'users.kyc.review' },
  { endpoint: '/admin/api/v1/market-feed/config', method: 'PATCH', permission: 'market.feed.write' },
  { endpoint: '/admin/api/v1/market-feed/reload', method: 'POST', permission: 'market.feed.operate' },
  { endpoint: '/admin/api/v1/market-feed/credentials/binance', method: 'PATCH', permission: 'market.feed.write' },
  { endpoint: '/admin/api/v1/new-coins/1/lifecycle', method: 'PATCH', permission: 'new_coin.projects.write' },
  { endpoint: '/admin/api/v1/new-coins/1/distribute', method: 'POST', permission: 'new_coin.projects.write' },
  { endpoint: '/admin/api/v1/new-coins/1/unlock-rule', method: 'PATCH', permission: 'new_coin.projects.write' },
  { endpoint: '/admin/api/v1/new-coins/1/unlock-fee-rule', method: 'PATCH', permission: 'new_coin.projects.write' },
  { endpoint: '/admin/api/v1/platform/brand', method: 'PATCH', permission: 'system.brand.write' },
  { endpoint: '/admin/api/v1/prediction/markets/1', method: 'PATCH', permission: 'prediction.markets.write' },
  { endpoint: '/admin/api/v1/prediction/markets/1/settle', method: 'POST', permission: 'prediction.markets.settle' },
  { endpoint: '/admin/api/v1/quick-recharge/config/test', method: 'POST', permission: 'wallet.quick_recharge.write' },
  { endpoint: '/admin/api/v1/quick-recharge/config', method: 'PATCH', permission: 'wallet.quick_recharge.write' },
  { endpoint: '/admin/api/v1/security-policy', method: 'PATCH', permission: 'system.security.write' },
  { endpoint: '/admin/api/v1/upload/config', method: 'PATCH', permission: 'system.uploads.write' },
  { endpoint: '/admin/api/v1/uploads/images', method: 'POST', permission: 'system.uploads.write' },
  { endpoint: '/admin/api/v1/prediction/settings', method: 'PATCH', permission: 'prediction.settings.write' },
  { endpoint: '/admin/api/v1/prediction/asset-configs', method: 'POST', permission: 'prediction.assets.write' },
  { endpoint: '/admin/api/v1/prediction/sync', method: 'POST', permission: 'prediction.sync.operate' },
  { endpoint: '/admin/api/v1/smtp/configs', method: 'POST', permission: 'system.smtp.write' },
  { endpoint: '/admin/api/v1/smtp/configs/1', method: 'PATCH', permission: 'system.smtp.write' },
  { endpoint: '/admin/api/v1/smtp/delivery-settings', method: 'PATCH', permission: 'system.smtp.write' },
  { endpoint: '/admin/api/v1/smtp/test', method: 'POST', permission: 'system.smtp.write' },
  { endpoint: '/admin/api/v1/market-strategies/1/kline-recovery/preview', method: 'POST', permission: 'market.strategies.write' },
  { endpoint: '/admin/api/v1/market-strategies/1/kline-recovery/execute', method: 'POST', permission: 'market.strategies.write' },
  { endpoint: '/admin/api/v1/market-strategies/1/versions/2/restore', method: 'POST', permission: 'market.strategies.operate' },
  { endpoint: '/admin/api/v1/market-strategies/preview', method: 'POST', permission: 'market.strategies.write' },
  { endpoint: '/admin/api/v1/market-strategies/1/status', method: 'PATCH', permission: 'market.strategies.write' },
  { endpoint: '/admin/api/v1/agent-commission-rules', method: 'POST', permission: 'agents.commission_rules.write' },
  { endpoint: '/admin/api/v1/agent-commissions/batch-status', method: 'POST', permission: 'agents.commissions.write' },
  { endpoint: '/admin/api/v1/agent-commissions/1/status', method: 'PATCH', permission: 'agents.commissions.write' },
  { endpoint: '/admin/api/v1/convert/pairs/1', method: 'DELETE', permission: 'convert.pairs.write' },
  { endpoint: '/admin/api/v1/earn/categories/1/status', method: 'PATCH', permission: 'earn.categories.write' },
  { endpoint: '/admin/api/v1/earn/products/1', method: 'PATCH', permission: 'earn.products.write' },
  { endpoint: '/admin/api/v1/loan/products/1/status', method: 'PATCH', permission: 'loan.products.write' },
  { endpoint: '/admin/api/v1/loan/orders/1/approve', method: 'POST', permission: 'loan.orders.review' },
  { endpoint: '/admin/api/v1/loan/orders/1/reject', method: 'POST', permission: 'loan.orders.review' },
  { endpoint: '/admin/api/v1/margin/products/1', method: 'PATCH', permission: 'margin.products.write' },
  { endpoint: '/admin/api/v1/market-pairs/1/status', method: 'PATCH', permission: 'market.pairs.write' },
  { endpoint: '/admin/api/v1/spot/orders/1/cancel', method: 'POST', permission: 'spot.orders.write' },
  { endpoint: '/admin/api/v1/news/1/status', method: 'PATCH', permission: 'content.news.write' },
  { endpoint: '/admin/api/v1/risk/rules/1/status', method: 'PATCH', permission: 'risk.rules.write' },
  { endpoint: '/admin/api/v1/seconds-contracts/products/1', method: 'DELETE', permission: 'seconds.products.write' },
  { endpoint: '/admin/api/v1/seconds-contracts/orders/1/settle', method: 'POST', permission: 'seconds.orders.settle' },
  { endpoint: '/admin/api/v1/countries/1/status', method: 'PATCH', permission: 'system.countries.write' },
  { endpoint: '/admin/api/v1/users/1/recharge', method: 'POST', permission: 'users.write' },
  { endpoint: '/admin/api/v1/users/1/2fa/reset', method: 'POST', permission: 'users.write' },
  { endpoint: '/admin/api/v1/assets/1', method: 'PATCH', permission: 'wallet.assets.write' },
  { endpoint: '/admin/api/v1/deposit-network-configs/1', method: 'PATCH', permission: 'wallet.networks.write' },
  { endpoint: '/admin/api/v1/deposit-address-pool/1/reclaim', method: 'POST', permission: 'wallet.address_pool.write' },
  { endpoint: '/admin/api/v1/wallet/withdrawals/1/broadcast', method: 'POST', permission: 'wallet.withdrawals.write' },
  { endpoint: '/admin/api/v1/wallet/withdrawals/1/approve', method: 'POST', permission: 'wallet.withdrawals.review' },
  { endpoint: '/admin/api/v1/wallet/withdrawals/1/reject', method: 'POST', permission: 'wallet.withdrawals.review' },
  { endpoint: '/admin/api/v1/wallet/withdrawals/1/confirm', method: 'POST', permission: 'wallet.withdrawals.review' },
  { endpoint: '/admin/api/v1/wallet/withdrawals/1/fail', method: 'POST', permission: 'wallet.withdrawals.review' },
  { endpoint: '/admin/api/v1/wallet/deposits/1/reverse', method: 'POST', permission: 'wallet.deposits.write' },
  { endpoint: '/admin/api/v1/quick-recharge/orders/A-1', method: 'DELETE', permission: 'wallet.quick_recharge.write' },
  { endpoint: '/admin/api/v1/auth/2fa/setup', method: 'POST', permission: null },
  { endpoint: '/admin/api/v1/access/me', method: 'GET', permission: null },
  { endpoint: '/admin/api/v1/not-mapped', method: 'POST', permission: 'admin.unmapped.write' }
];

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
    expect(adminReadPermissionForPath('/admin/prediction/assets')).toBe('prediction.assets.read');
    expect(adminReadPermissionForPath('/admin/account/security')).toBe('account.security.read');
    expect(adminReadPermissionForPath('/admin/config-center')).toBe('config_center.read');
    expect(adminReadPermissionForPath('/admin/support')).toBe('support.conversations.read');
    expect(adminReadPermissionForPath('/admin/not-mapped')).toBe('admin.unmapped.read');
  });

  it('maps resource endpoints and fails closed for unknown endpoints', () => {
    expect(adminPermissionForRequest('/admin/api/v1/market-pairs', 'POST')).toBe('market.pairs.write');
    expect(adminPermissionForRequest('/admin/api/v1/new-coins', 'POST')).toBe('new_coin.projects.write');
    expect(adminPermissionForRequest('/admin/api/v1/config-center', 'POST')).toBe('config_center.write');
    expect(adminPermissionForRequest('/admin/api/v1/support/conversations/11/messages', 'POST')).toBe('support.conversations.write');
    expect(adminPermissionForRequest('/admin/api/v1/not-mapped', 'POST')).toBe('admin.unmapped.write');
  });

  it('对所有 Admin 动作描述符使用与后端一致的单一权限', () => {
    actionDescriptorMatrix.forEach(({ endpoint, method, permission }) => {
      expect(adminPermissionForRequest(endpoint, method), `${method} ${endpoint}`).toBe(permission);
    });
  });

  it('完整角色矩阵严格区分 read/write/review/operate/settle，不以权限并集放行', () => {
    const actions = ['read', 'write', 'review', 'operate', 'settle'] as const;
    actionDescriptorMatrix
      .filter((descriptor): descriptor is ActionDescriptor & { permission: string } => descriptor.permission !== null)
      .forEach(({ endpoint, method, permission }) => {
        const resource = permission.slice(0, permission.lastIndexOf('.'));
        const expectedAction = permission.slice(permission.lastIndexOf('.') + 1);
        actions.forEach((roleAction) => {
          expect(
            hasAdminPermission(access([`${resource}.${roleAction}`]), adminPermissionForRequest(endpoint, method)!),
            `${resource}.${roleAction} -> ${method} ${endpoint}`
          ).toBe(roleAction === expectedAction);
        });
        expect(hasAdminPermission(access([`${resource}.*`]), permission)).toBe(true);

        const readPermission = adminPermissionForRequest(endpoint, 'GET');
        expect(readPermission).toBe(`${resource}.read`);
        actions.forEach((roleAction) => {
          expect(hasAdminPermission(access([`${resource}.${roleAction}`]), readPermission!)).toBe(roleAction === 'read');
        });
      });
  });

  it('动作分类顺序优先复核，其次结算和运行操作', () => {
    expect(adminActionForRequest('/admin/api/v1/demo/approve/settle', 'POST')).toBe('review');
    expect(adminActionForRequest('/admin/api/v1/demo/settle', 'POST')).toBe('settle');
    expect(adminActionForRequest('/admin/api/v1/demo/recovery/preview', 'POST')).toBe('operate');
    expect(adminActionForRequest('/admin/api/v1/demo', 'GET')).toBe('read');
  });

  it('动作级 UI 门严格只显示当前角色的单一动作', () => {
    const cases = [
      { action: 'read', endpoint: '/admin/api/v1/market-strategies/1', method: 'GET' },
      { action: 'write', endpoint: '/admin/api/v1/market-strategies/1', method: 'PATCH' },
      { action: 'review', endpoint: '/admin/api/v1/market-strategies/1/approve', method: 'POST' },
      { action: 'settle', endpoint: '/admin/api/v1/market-strategies/1/settle', method: 'POST' },
      { action: 'operate', endpoint: '/admin/api/v1/market-strategies/1/recovery/preview', method: 'POST' }
    ] as const;

    cases.forEach((roleCase) => {
      render(
        <AdminAccessProvider access={access([`market.strategies.${roleCase.action}`])}>
          {cases.map((item) => (
            <AdminRequestActionBoundary endpoint={item.endpoint} key={item.action} method={item.method}>
              <span>{item.action}</span>
            </AdminRequestActionBoundary>
          ))}
        </AdminAccessProvider>
      );
      expect(screen.getByText(roleCase.action)).toBeInTheDocument();
      cases.filter((item) => item.action !== roleCase.action).forEach((item) => {
        expect(screen.queryByText(item.action)).not.toBeInTheDocument();
      });
      cleanup();
    });
  });
});
