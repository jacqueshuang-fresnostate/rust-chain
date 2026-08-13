import { Navigate, type RouteObject } from 'react-router-dom';

type ResourceConfigsModule = typeof import('./resources/resourceConfigs');
type ResourceConfigKey = keyof ResourceConfigsModule['resourceConfigs'];

// 后台页面全部走路由级按需加载：登录页与代理端不再打包管理端代码，
// 43 个资源页共享同一份 resourceConfigs chunk，独立配置页各自成块。
// handle.resourceKey 让路由与资源配置的绑定关系保持静态可读，测试无需触发动态导入即可校验。
function resourceRoute(path: string, resourceKey: ResourceConfigKey): RouteObject {
  return {
    path,
    handle: { resourceKey },
    lazy: async () => {
      const { ResourcePage, resourceConfigs } = await import('./resources/resourceConfigs');
      return {
        Component: function AdminResourceRoute() {
          return <ResourcePage config={resourceConfigs[resourceKey]} />;
        }
      };
    }
  };
}

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  {
    path: 'dashboard',
    lazy: async () => ({ Component: (await import('./dashboard/DashboardPage')).DashboardPage })
  },
  resourceRoute('users', 'users'),
  {
    path: 'users/kyc',
    lazy: async () => ({ Component: (await import('./actions/KycManagementPage')).KycManagementPage })
  },
  {
    path: 'agents',
    lazy: async () => ({ Component: (await import('./actions/AgentManagementPage')).AgentManagementPage })
  },
  resourceRoute('agent-commissions', 'agentCommissions'),
  resourceRoute('agent-commission-rules', 'agentCommissionRules'),
  resourceRoute('news', 'news'),
  resourceRoute('assets', 'assets'),
  resourceRoute('wallet/accounts', 'walletAccounts'),
  resourceRoute('wallet/deposit-network-configs', 'depositNetworkConfigs'),
  resourceRoute('wallet/deposit-address-pool', 'depositAddressPool'),
  {
    path: 'wallet/quick-recharge',
    lazy: async () => ({
      Component: (await import('./actions/QuickRechargeConfigPage')).QuickRechargeConfigPage
    })
  },
  resourceRoute('wallet/quick-recharge-orders', 'quickRechargeOrders'),
  resourceRoute('wallet/deposits', 'walletDeposits'),
  resourceRoute('wallet/withdrawals', 'walletWithdrawals'),
  resourceRoute('wallet/ledger', 'walletLedger'),
  resourceRoute('loan/products', 'loanProducts'),
  resourceRoute('loan/orders', 'loanOrders'),
  {
    path: 'prediction/settings',
    lazy: async () => ({
      Component: (await import('./actions/PredictionConfigPage')).PredictionConfigPage
    })
  },
  resourceRoute('prediction/assets', 'predictionAssetConfigs'),
  resourceRoute('prediction/markets', 'predictionMarkets'),
  resourceRoute('prediction/orders', 'predictionOrders'),
  resourceRoute('prediction/sync-logs', 'predictionSyncLogs'),
  resourceRoute('spot/orders', 'spotOrders'),
  resourceRoute('spot/trades', 'spotTrades'),
  resourceRoute('new-coins/projects', 'newCoinProjects'),
  {
    path: 'new-coins/actions',
    lazy: async () => ({ Component: (await import('./actions/NewCoinActions')).NewCoinActions })
  },
  resourceRoute('new-coins/subscriptions', 'newCoinSubscriptions'),
  resourceRoute('new-coins/distributions', 'newCoinDistributions'),
  resourceRoute('new-coins/purchases', 'newCoinPurchases'),
  resourceRoute('new-coins/lock-positions', 'newCoinLockPositions'),
  resourceRoute('new-coins/unlocks', 'newCoinUnlocks'),
  resourceRoute('market/pairs', 'marketPairs'),
  resourceRoute('market/strategies', 'marketStrategies'),
  {
    path: 'market/strategies/actions',
    lazy: async () => ({
      Component: (await import('./actions/MarketStrategyActions')).MarketStrategyActions
    })
  },
  {
    path: 'market/feed-config',
    lazy: async () => ({
      Component: (await import('./actions/MarketFeedConfigPage')).MarketFeedConfigPage
    })
  },
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
  {
    path: 'system/security-policy',
    lazy: async () => ({ Component: (await import('./actions/SecurityPolicyPage')).SecurityPolicyPage })
  },
  {
    path: 'system/two-factor',
    lazy: async () => ({ Component: (await import('./actions/AdminTwoFactorPage')).AdminTwoFactorPage })
  },
  {
    path: 'system/brand',
    lazy: async () => ({ Component: (await import('./actions/PlatformBrandPage')).PlatformBrandPage })
  },
  {
    path: 'system/smtp',
    lazy: async () => ({ Component: (await import('./actions/SmtpConfigPage')).SmtpConfigPage })
  },
  {
    path: 'system/uploads',
    lazy: async () => ({ Component: (await import('./actions/UploadConfigPage')).UploadConfigPage })
  },
  resourceRoute('audit-logs', 'auditLogs')
];
