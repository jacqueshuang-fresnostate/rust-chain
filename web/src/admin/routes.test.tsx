import type { ComponentType, ReactElement } from 'react';
import type { RouteObject } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { adminRoutes } from './routes';

function findRoute(path: string): RouteObject | undefined {
  return adminRoutes.find((candidate) => candidate.path === path);
}

// 资源路由把绑定的配置键静态暴露在 handle 上，因此不必触发动态导入即可断言绑定关系。
function routeResourceKey(path: string) {
  const handle = findRoute(path)?.handle as { resourceKey?: string } | undefined;
  return handle?.resourceKey ?? '';
}

function routePermission(path: string) {
  const handle = findRoute(path)?.handle as { permission?: string } | undefined;
  return handle?.permission ?? '';
}

// 独立配置页只有解析 lazy 才能拿到组件，解析结果同时验证目标模块确实导出了该组件。
async function lazyComponentName(path: string) {
  const route = findRoute(path);
  if (typeof route?.lazy !== 'function') {
    return '';
  }

  const resolved = (await route.lazy()) as { Component?: ComponentType };
  return resolved.Component?.name ?? '';
}

describe('adminRoutes', () => {
  it.each([
    ['news', 'news'],
    ['system/countries', 'countries'],
    ['new-coins/subscriptions', 'newCoinSubscriptions'],
    ['new-coins/distributions', 'newCoinDistributions'],
    ['users', 'users'],
    ['agent-commission-rules', 'agentCommissionRules'],
    ['assets', 'assets'],
    ['wallet/accounts', 'walletAccounts'],
    ['wallet/deposit-network-configs', 'depositNetworkConfigs'],
    ['wallet/deposit-address-pool', 'depositAddressPool'],
    ['wallet/quick-recharge-orders', 'quickRechargeOrders'],
    ['wallet/ledger', 'walletLedger'],
    ['loan/products', 'loanProducts'],
    ['loan/orders', 'loanOrders'],
    ['earn/categories', 'earnCategories'],
    ['risk', 'riskRules'],
    ['risk/events', 'riskEvents']
  ])('binds resource page %s to config %s', (path, expectedKey) => {
    expect(routeResourceKey(path)).toBe(expectedKey);
  });

  // 解析 lazy 会真实转换体量很大的 resourceConfigs 模块，首次导入在并行负载下远超默认超时。
  it(
    'lazily loads every resource route through the shared resource page',
    async () => {
      const resourceRoutes = adminRoutes.filter((route) => Boolean((route.handle as { resourceKey?: string } | undefined)?.resourceKey));
      // Two lock/release resources now share a permission-scoped tab workbench.
      expect(resourceRoutes.length).toBeGreaterThanOrEqual(38);
      resourceRoutes.forEach((route) => {
        expect(typeof route.lazy).toBe('function');
        expect(route.element).toBeUndefined();
      });

      expect(await lazyComponentName('users')).toBe('AdminResourceRoute');
    },
    120_000
  );

  it.each([
    ['market/feed-config', 'MarketFeedConfigPage'],
    ['users/kyc/settings', 'KycSettingsPage'],
    ['users/kyc/reviews', 'KycReviewsPage'],
    ['system/smtp', 'SmtpConfigPage'],
    ['system/uploads', 'UploadConfigPage'],
    ['system/brand', 'PlatformBrandPage'],
    ['wallet/quick-recharge', 'QuickRechargeConfigPage'],
    ['system/security-policy', 'SecurityPolicyPage'],
    ['account/security', 'AdminTwoFactorPage'],
    ['agents', 'AgentManagementPage'],
    ['dashboard', 'DashboardPage'],
    ['config-center', 'ConfigCenterPage'],
    ['support', 'AdminSupportPage'],
    ['audit-logs', 'AuditLogsPage'],
    ['new-coins/actions', 'NewCoinActions'],
    ['new-coins/projects/:projectId', 'NewCoinProjectPage'],
    ['new-coins/lock-positions', 'NewCoinLocksPage'],
    ['new-coins/unlocks', 'NewCoinLocksPage'],
    ['prediction/settings', 'PredictionSettingsPage'],
    ['prediction/sync', 'PredictionSyncPage']
  ])(
    'registers the %s action page',
    async (path, expectedName) => {
      expect(await lazyComponentName(path)).toBe(expectedName);
    },
    120_000
  );

  it('keeps legacy workflow URLs as eager compatibility redirects', () => {
    const eagerRoutes = adminRoutes.filter((route) => route.path && route.element);
    expect(eagerRoutes.map((route) => route.path)).toEqual([
      'users/kyc',
      'prediction/assets',
      'prediction/sync-logs',
      'market/strategies/actions',
      'system/two-factor'
    ]);
    expect(
      Object.fromEntries(
        eagerRoutes.map((route) => [
          route.path,
          (route.element as ReactElement<{ replace?: boolean; to?: string }>).props
        ])
      )
    ).toMatchObject({
      'users/kyc': { replace: true, to: '/admin/users/kyc/reviews' },
      'prediction/assets': { replace: true, to: '/admin/prediction/settings?tab=assets' },
      'prediction/sync-logs': { replace: true, to: '/admin/prediction/sync' },
      'market/strategies/actions': { replace: true, to: '/admin/market/strategies' },
      'system/two-factor': { replace: true, to: '/admin/account/security' }
    });
  });

  it.each([
    ['users/kyc/settings', 'users.kyc.read'],
    ['users/kyc/reviews', 'users.kyc.read'],
    ['prediction/settings', 'prediction.settings.read'],
    ['prediction/sync', 'prediction.sync.read'],
    ['account/security', 'account.security.read'],
    ['config-center', 'config_center.read'],
    ['support', 'support.conversations.read'],
    ['audit-logs', 'audit.logs.read']
  ])('keeps the %s route behind %s', (path, permission) => {
    expect(routePermission(path)).toBe(permission);
  });

  it('uses the dedicated audit workbench instead of the generic resource page', () => {
    expect(routeResourceKey('audit-logs')).toBe('');
  });

  it.each([
    'margin/actions',
    'spot/actions',
    'seconds-contract/actions',
    'earn/actions',
    'convert/rules'
  ])('does not register the removed %s route', (path) => {
    expect(findRoute(path)).toBeUndefined();
  });
});
