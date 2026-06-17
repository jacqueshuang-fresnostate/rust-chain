import { Navigate, type RouteObject } from 'react-router-dom';

import { AgentManagementPage } from './actions/AgentManagementPage';
import { KycManagementPage } from './actions/KycManagementPage';
import { MarketFeedConfigPage } from './actions/MarketFeedConfigPage';
import { MarketStrategyActions } from './actions/MarketStrategyActions';
import { NewCoinActions } from './actions/NewCoinActions';
import { PlatformBrandPage } from './actions/PlatformBrandPage';
import { PredictionConfigPage } from './actions/PredictionConfigPage';
import { QuickRechargeConfigPage } from './actions/QuickRechargeConfigPage';
import { SecurityPolicyPage } from './actions/SecurityPolicyPage';
import { SmtpConfigPage } from './actions/SmtpConfigPage';
import { UploadConfigPage } from './actions/UploadConfigPage';
import { DashboardPage } from './dashboard/DashboardPage';
import { ResourcePage, resourceConfigs } from './resources/resourceConfigs';

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  { path: 'dashboard', element: <DashboardPage /> },
  { path: 'users', element: <ResourcePage config={resourceConfigs.users} /> },
  { path: 'users/kyc', element: <KycManagementPage /> },
  { path: 'agents', element: <AgentManagementPage /> },
  { path: 'agent-commissions', element: <ResourcePage config={resourceConfigs.agentCommissions} /> },
  { path: 'agent-commission-rules', element: <ResourcePage config={resourceConfigs.agentCommissionRules} /> },
  { path: 'news', element: <ResourcePage config={resourceConfigs.news} /> },
  { path: 'assets', element: <ResourcePage config={resourceConfigs.assets} /> },
  { path: 'wallet/accounts', element: <ResourcePage config={resourceConfigs.walletAccounts} /> },
  { path: 'wallet/deposit-address-pool', element: <ResourcePage config={resourceConfigs.depositAddressPool} /> },
  { path: 'wallet/quick-recharge', element: <QuickRechargeConfigPage /> },
  { path: 'wallet/quick-recharge-orders', element: <ResourcePage config={resourceConfigs.quickRechargeOrders} /> },
  { path: 'wallet/ledger', element: <ResourcePage config={resourceConfigs.walletLedger} /> },
  { path: 'loan/products', element: <ResourcePage config={resourceConfigs.loanProducts} /> },
  { path: 'loan/orders', element: <ResourcePage config={resourceConfigs.loanOrders} /> },
  { path: 'prediction/settings', element: <PredictionConfigPage /> },
  { path: 'prediction/assets', element: <ResourcePage config={resourceConfigs.predictionAssetConfigs} /> },
  { path: 'prediction/markets', element: <ResourcePage config={resourceConfigs.predictionMarkets} /> },
  { path: 'prediction/orders', element: <ResourcePage config={resourceConfigs.predictionOrders} /> },
  { path: 'prediction/sync-logs', element: <ResourcePage config={resourceConfigs.predictionSyncLogs} /> },
  { path: 'spot/orders', element: <ResourcePage config={resourceConfigs.spotOrders} /> },
  { path: 'spot/trades', element: <ResourcePage config={resourceConfigs.spotTrades} /> },
  { path: 'new-coins/projects', element: <ResourcePage config={resourceConfigs.newCoinProjects} /> },
  { path: 'new-coins/actions', element: <NewCoinActions /> },
  { path: 'new-coins/subscriptions', element: <ResourcePage config={resourceConfigs.newCoinSubscriptions} /> },
  { path: 'new-coins/distributions', element: <ResourcePage config={resourceConfigs.newCoinDistributions} /> },
  { path: 'new-coins/purchases', element: <ResourcePage config={resourceConfigs.newCoinPurchases} /> },
  { path: 'new-coins/lock-positions', element: <ResourcePage config={resourceConfigs.newCoinLockPositions} /> },
  { path: 'new-coins/unlocks', element: <ResourcePage config={resourceConfigs.newCoinUnlocks} /> },
  { path: 'market/pairs', element: <ResourcePage config={resourceConfigs.marketPairs} /> },
  { path: 'market/strategies', element: <ResourcePage config={resourceConfigs.marketStrategies} /> },
  { path: 'market/strategies/actions', element: <MarketStrategyActions /> },
  { path: 'market/feed-config', element: <MarketFeedConfigPage /> },
  { path: 'convert/pairs', element: <ResourcePage config={resourceConfigs.convertPairs} /> },
  { path: 'convert/orders', element: <ResourcePage config={resourceConfigs.convertOrders} /> },
  { path: 'seconds-contract/products', element: <ResourcePage config={resourceConfigs.secondsProducts} /> },
  { path: 'seconds-contract/orders', element: <ResourcePage config={resourceConfigs.secondsOrders} /> },
  { path: 'margin/products', element: <ResourcePage config={resourceConfigs.marginProducts} /> },
  { path: 'margin/positions', element: <ResourcePage config={resourceConfigs.marginPositions} /> },
  { path: 'margin/liquidations', element: <ResourcePage config={resourceConfigs.marginLiquidations} /> },
  { path: 'margin/interest', element: <ResourcePage config={resourceConfigs.marginInterest} /> },
  { path: 'earn/categories', element: <ResourcePage config={resourceConfigs.earnCategories} /> },
  { path: 'earn/products', element: <ResourcePage config={resourceConfigs.earnProducts} /> },
  { path: 'earn/subscriptions', element: <ResourcePage config={resourceConfigs.earnSubscriptions} /> },
  { path: 'risk', element: <ResourcePage config={resourceConfigs.riskRules} /> },
  { path: 'risk/events', element: <ResourcePage config={resourceConfigs.riskEvents} /> },
  { path: 'system/countries', element: <ResourcePage config={resourceConfigs.countries} /> },
  { path: 'system/security-policy', element: <SecurityPolicyPage /> },
  { path: 'system/brand', element: <PlatformBrandPage /> },
  { path: 'system/smtp', element: <SmtpConfigPage /> },
  { path: 'system/uploads', element: <UploadConfigPage /> },
  { path: 'audit-logs', element: <ResourcePage config={resourceConfigs.auditLogs} /> }
];
