import type { ComponentType } from 'react';
import { Navigate, type RouteObject } from 'react-router-dom';

import { AdminPermissionBoundary, adminReadPermissionForPath } from './access';

type ResourceConfigsModule = typeof import('./resources/resourceConfigs');
type ResourceConfigKey = keyof ResourceConfigsModule['resourceConfigs'];

// 后台页面全部走路由级按需加载：登录页与代理端不再打包管理端代码，
// 通用资源页共享同一份 resourceConfigs chunk，独立配置页各自成块。
// handle.resourceKey 让路由与资源配置的绑定关系保持静态可读，测试无需触发动态导入即可校验。
function resourceRoute(path: string, resourceKey: ResourceConfigKey): RouteObject {
  const permission = adminReadPermissionForPath(`/admin/${path}`);
  return {
    path,
    handle: { resourceKey, permission },
    lazy: async () => {
      const { ResourcePage, resourceConfigs } = await import('./resources/resourceConfigs');
      const Page = path.startsWith('new-coins/') ? (await import('./new-coins/NewCoinResourcePage')).NewCoinResourcePage : ResourcePage;
      return {
        Component: function AdminResourceRoute() {
          return (
            <AdminPermissionBoundary permission={permission}>
              <Page config={resourceConfigs[resourceKey]} />
            </AdminPermissionBoundary>
          );
        }
      };
    }
  };
}

/** 独立后台页面同样在路由入口校验读取权限，避免仅靠侧栏隐藏形成前端越权入口。 */
function guardedLazyRoute(path: string, load: () => Promise<ComponentType>): RouteObject {
  const permission = adminReadPermissionForPath(`/admin/${path}`);
  return {
    path,
    handle: { permission },
    lazy: async () => {
      const Component = await load();
      const GuardedAdminPage = function GuardedAdminPage() {
        return (
          <AdminPermissionBoundary permission={permission}>
            <Component />
          </AdminPermissionBoundary>
        );
      };
      // 保留被包装页面的函数名，方便路由注册测试和开发工具识别真实业务页面。
      Object.defineProperty(GuardedAdminPage, 'name', { value: Component.name });
      return {
        Component: GuardedAdminPage
      };
    }
  };
}

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  guardedLazyRoute('dashboard', async () => (await import('./dashboard/DashboardPage')).DashboardPage),
  guardedLazyRoute(
    'config-center',
    async () => (await import('./config-center/ConfigCenterPage')).ConfigCenterPage
  ),
  guardedLazyRoute(
    'support',
    async () => (await import('./support/AdminSupportPage')).AdminSupportPage
  ),
  resourceRoute('users', 'users'),
  guardedLazyRoute(
    'users/kyc/settings',
    async () => (await import('./actions/KycManagementPage')).KycSettingsPage
  ),
  guardedLazyRoute(
    'users/kyc/reviews',
    async () => (await import('./actions/KycManagementPage')).KycReviewsPage
  ),
  { path: 'users/kyc', element: <Navigate to="/admin/users/kyc/reviews" replace /> },
  guardedLazyRoute('agents', async () => (await import('./actions/AgentManagementPage')).AgentManagementPage),
  resourceRoute('agent-commissions', 'agentCommissions'),
  resourceRoute('agent-commission-rules', 'agentCommissionRules'),
  resourceRoute('news', 'news'),
  resourceRoute('assets', 'assets'),
  resourceRoute('wallet/accounts', 'walletAccounts'),
  resourceRoute('wallet/deposit-network-configs', 'depositNetworkConfigs'),
  resourceRoute('wallet/deposit-address-pool', 'depositAddressPool'),
  guardedLazyRoute(
    'wallet/quick-recharge',
    async () => (await import('./actions/QuickRechargeConfigPage')).QuickRechargeConfigPage
  ),
  resourceRoute('wallet/quick-recharge-orders', 'quickRechargeOrders'),
  resourceRoute('wallet/deposits', 'walletDeposits'),
  resourceRoute('wallet/withdrawals', 'walletWithdrawals'),
  resourceRoute('wallet/ledger', 'walletLedger'),
  resourceRoute('loan/products', 'loanProducts'),
  resourceRoute('loan/orders', 'loanOrders'),
  guardedLazyRoute(
    'prediction/settings',
    async () => (await import('./actions/PredictionConfigPage')).PredictionSettingsPage
  ),
  {
    path: 'prediction/assets',
    element: <Navigate to="/admin/prediction/settings?tab=assets" replace />
  },
  resourceRoute('prediction/markets', 'predictionMarkets'),
  resourceRoute('prediction/orders', 'predictionOrders'),
  guardedLazyRoute(
    'prediction/sync',
    async () => (await import('./actions/PredictionConfigPage')).PredictionSyncPage
  ),
  { path: 'prediction/sync-logs', element: <Navigate to="/admin/prediction/sync" replace /> },
  resourceRoute('spot/orders', 'spotOrders'),
  resourceRoute('spot/trades', 'spotTrades'),
  resourceRoute('new-coins/projects', 'newCoinProjects'),
  guardedLazyRoute('new-coins/projects/:projectId', async () => (await import('./new-coins/NewCoinProjectPage')).NewCoinProjectPage),
  guardedLazyRoute('new-coins/actions', async () => (await import('./actions/NewCoinActions')).NewCoinActions),
  resourceRoute('new-coins/subscriptions', 'newCoinSubscriptions'),
  resourceRoute('new-coins/distributions', 'newCoinDistributions'),
  resourceRoute('new-coins/purchases', 'newCoinPurchases'),
  guardedLazyRoute('new-coins/lock-positions', async () => (await import('./new-coins/NewCoinResourcePage')).NewCoinLocksPage),
  guardedLazyRoute('new-coins/unlocks', async () => (await import('./new-coins/NewCoinResourcePage')).NewCoinLocksPage),
  resourceRoute('market/pairs', 'marketPairs'),
  resourceRoute('market/strategies', 'marketStrategies'),
  {
    path: 'market/strategies/actions',
    element: <Navigate to="/admin/market/strategies" replace />
  },
  guardedLazyRoute(
    'market/feed-config',
    async () => (await import('./actions/MarketFeedConfigPage')).MarketFeedConfigPage
  ),
  resourceRoute('convert/pairs', 'convertPairs'),
  resourceRoute('convert/orders', 'convertOrders'),
  resourceRoute('seconds-contract/products', 'secondsProducts'),
  resourceRoute('seconds-contract/orders', 'secondsOrders'),
  resourceRoute('margin/products', 'marginProducts'),
  resourceRoute('margin/positions', 'marginPositions'),
  resourceRoute('margin/liquidations', 'marginLiquidations'),
  resourceRoute('margin/interest', 'marginInterest'),
  resourceRoute('earn/categories', 'earnCategories'),
  resourceRoute('earn/products', 'earnProducts'),
  resourceRoute('earn/subscriptions', 'earnSubscriptions'),
  resourceRoute('risk', 'riskRules'),
  resourceRoute('risk/events', 'riskEvents'),
  resourceRoute('system/countries', 'countries'),
  guardedLazyRoute(
    'system/security-policy',
    async () => (await import('./actions/SecurityPolicyPage')).SecurityPolicyPage
  ),
  guardedLazyRoute(
    'account/security',
    async () => (await import('./actions/AdminTwoFactorPage')).AdminTwoFactorPage
  ),
  { path: 'system/two-factor', element: <Navigate to="/admin/account/security" replace /> },
  guardedLazyRoute('system/brand', async () => (await import('./actions/PlatformBrandPage')).PlatformBrandPage),
  guardedLazyRoute('system/smtp', async () => (await import('./actions/SmtpConfigPage')).SmtpConfigPage),
  guardedLazyRoute('system/uploads', async () => (await import('./actions/UploadConfigPage')).UploadConfigPage),
  guardedLazyRoute('audit-logs', async () => (await import('./audit/AuditLogsPage')).AuditLogsPage)
];
