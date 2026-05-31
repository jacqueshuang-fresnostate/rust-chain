import { Navigate, type RouteObject } from 'react-router-dom';

import { AgentManagementPage } from './actions/AgentManagementPage';
import { ConvertRuleActions } from './actions/ConvertRuleActions';
import { MarketFeedConfigPage } from './actions/MarketFeedConfigPage';
import { MarketStrategyActions } from './actions/MarketStrategyActions';
import { NewCoinActions } from './actions/NewCoinActions';
import { ProductStatusActions } from './actions/ProductStatusActions';
import { DashboardPage } from './dashboard/DashboardPage';
import { ResourcePage, resourceConfigs } from './resources/resourceConfigs';

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  { path: 'dashboard', element: <DashboardPage /> },
  { path: 'users', element: <ResourcePage config={resourceConfigs.users} /> },
  { path: 'agents', element: <AgentManagementPage /> },
  { path: 'agent-commissions', element: <ResourcePage config={resourceConfigs.agentCommissions} /> },
  { path: 'wallet/accounts', element: <ResourcePage config={resourceConfigs.walletAccounts} /> },
  { path: 'wallet/ledger', element: <ResourcePage config={resourceConfigs.walletLedger} /> },
  { path: 'spot/actions', element: <ProductStatusActions /> },
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
  { path: 'convert/rules', element: <ConvertRuleActions /> },
  { path: 'convert/orders', element: <ResourcePage config={resourceConfigs.convertOrders} /> },
  { path: 'seconds-contract/products', element: <ResourcePage config={resourceConfigs.secondsProducts} /> },
  { path: 'seconds-contract/orders', element: <ResourcePage config={resourceConfigs.secondsOrders} /> },
  { path: 'seconds-contract/actions', element: <ProductStatusActions /> },
  { path: 'margin/products', element: <ResourcePage config={resourceConfigs.marginProducts} /> },
  { path: 'margin/positions', element: <ResourcePage config={resourceConfigs.marginPositions} /> },
  { path: 'margin/liquidations', element: <ResourcePage config={resourceConfigs.marginLiquidations} /> },
  { path: 'margin/interest', element: <ResourcePage config={resourceConfigs.marginInterest} /> },
  { path: 'margin/actions', element: <ProductStatusActions /> },
  { path: 'earn/products', element: <ResourcePage config={resourceConfigs.earnProducts} /> },
  { path: 'earn/subscriptions', element: <ResourcePage config={resourceConfigs.earnSubscriptions} /> },
  { path: 'earn/actions', element: <ProductStatusActions /> },
  { path: 'risk', element: <ResourcePage config={resourceConfigs.riskRules} /> },
  { path: 'risk/events', element: <ResourcePage config={resourceConfigs.riskEvents} /> },
  { path: 'audit-logs', element: <ResourcePage config={resourceConfigs.auditLogs} /> }
];
