import { useEffect, useMemo, useState } from 'react';

import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import {
  AgentCommissionRowActions,
  AgentCommissionRuleRowActions,
  AdminNewsRowActions,
  AssetRowActions,
  ConvertOrderRowActions,
  ConvertPairRowActions,
  CreateAgentCommissionRuleAction,
  CreateAdminNewsAction,
  CreateAssetAction,
  CreateConvertPairAction,
  CreateEarnProductAction,
  CreateMarginPairAction,
  CreateMarketStrategyAction,
  CreateNewCoinProjectAction,
  CreateRiskRuleAction,
  CreateUserAction,
  CreateSecondsPairAction,
  CreateSpotPairAction,
  EarnProductRowActions,
  EarnSubscriptionRowActions,
  MarginLiquidationRowActions,
  MarginPositionRowActions,
  MarginProductRowActions,
  MarketPairRowActions,
  MarketStrategyRowActions,
  RiskRuleRowActions,
  SecondsOrderRowActions,
  SecondsProductRowActions,
  SpotOrderRowActions,
  UserRowActions
} from './ResourceCreateActions';
import { subscribeMarketTicker } from '../../api/marketTickerSocket';
import type { FilterField } from '../../shared/FilterBar';
import { formatAdminNumber } from '../../shared/numberFormat';
import type { ApiRecord } from '../../api/types';

export type ResourceConfig = {
  actions?: React.ComponentProps<typeof AdminResourcePage<ApiRecord>>['actions'];
  columns: Array<AdminResourceColumn<ApiRecord>>;
  endpoint: string;
  filters?: FilterField[];
  responseKey: string;
  rowActions?: React.ComponentProps<typeof AdminResourcePage<ApiRecord>>['rowActions'];
  showJsonAction?: boolean;
  title: string;
};

const limitFilter: FilterField = { key: 'limit', label: '数量限制' };
const statusFilter: FilterField = { key: 'status', label: '状态' };
const userFilter: FilterField = { key: 'user_id', label: '用户ID' };
const emailFilter: FilterField = { key: 'email', label: '邮箱' };
const pairFilter: FilterField = { key: 'pair_id', label: '交易对ID' };
const projectFilter: FilterField = { key: 'project_id', label: '项目ID' };
const assetFilter: FilterField = { key: 'asset_id', label: '资产ID' };

const assetTypeLabels = {
  coin: '数字货币',
  stablecoin: '稳定币',
  fiat: '法币',
  platform: '平台币'
};
const statusOptions = [
  { label: '启用', value: 'active' },
  { label: '禁用', value: 'disabled' }
];
const agentCommissionStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: [
    { label: 'pending', value: 'pending' },
    { label: 'settled', value: 'settled' },
    { label: 'rejected', value: 'rejected' }
  ]
};
const agentCommissionRuleProductFilter: FilterField = { key: 'product_type', label: '产品类型', type: 'select', options: [{ label: 'convert', value: 'convert' }] };
const agentCommissionRuleStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: [
    { label: 'active', value: 'active' },
    { label: 'disabled', value: 'disabled' }
  ]
};
const assetTypeFilter: FilterField = { key: 'asset_type', label: '资产类型', type: 'select', options: Object.entries(assetTypeLabels).map(([value, label]) => ({ label, value })) };
const statusSelectFilter: FilterField = { key: 'status', label: '状态', type: 'select', options: statusOptions };

const marketTypeLabels = {
  external: '外部行情',
  internal: '内部撮合',
  strategy: '策略行情'
};

const marginModeLabels = {
  isolated: '逐仓',
  cross: '全仓'
};

const earnProductCategoryLabels = {
  fixed_term: '定期',
  flexible: '活期',
  structured: '结构化',
  staking: '质押'
};

const newsCategoryLabels = {
  general: '通用资讯',
  market: '市场资讯',
  product: '产品资讯',
  system: '系统公告',
  promotion: '活动推广'
};

const newsStatusOptions = [
  { label: '草稿', value: 'draft' },
  { label: '已发布', value: 'published' },
  { label: '已归档', value: 'archived' }
];

const newsCategoryFilter: FilterField = {
  key: 'category',
  label: '分类',
  type: 'select',
  options: Object.entries(newsCategoryLabels).map(([value, label]) => ({ label, value }))
};
const newsStatusFilter: FilterField = { key: 'status', label: '状态', type: 'select', options: newsStatusOptions };

function MarginLeverageLevels({ levels }: { levels: unknown }) {
  const normalizedLevels = Array.isArray(levels)
    ? levels
        .map((level) => (typeof level === 'number' || typeof level === 'string' ? formatAdminNumber(level) : null))
        .filter((level): level is string => Boolean(level))
    : [];

  return <span>{normalizedLevels.length > 0 ? normalizedLevels.map((level) => `${level}x`).join(' / ') : '-'}</span>;
}

const marketTypeOptions = Object.entries(marketTypeLabels).map(([value, label]) => ({ label, value }));
const marketPairStatusFilter: FilterField = statusSelectFilter;
const marketPairSymbolFilter: FilterField = { key: 'symbol', label: '交易对', type: 'select', optionsFromRows: true };
const marketTypeFilter: FilterField = { key: 'market_type', label: '市场类型', type: 'select', options: marketTypeOptions };

function normalizeTickerSymbol(symbol: string) {
  return symbol
    .trim()
    .split('')
    .filter((character) => /[A-Za-z0-9]/.test(character))
    .join('')
    .toUpperCase();
}

function MarketPairLatestPrice({ symbol }: { symbol: unknown }) {
  const normalizedSymbol = useMemo(() => (typeof symbol === 'string' ? normalizeTickerSymbol(symbol) : ''), [symbol]);
  const [latestPrice, setLatestPrice] = useState<string | null>(null);

  useEffect(() => {
    setLatestPrice(null);
    if (!normalizedSymbol) {
      return undefined;
    }

    return subscribeMarketTicker(normalizedSymbol, ({ lastPrice }) => setLatestPrice(lastPrice));
  }, [normalizedSymbol]);

  return <span>{formatAdminNumber(latestPrice) ?? '-'}</span>;
}

export const resourceConfigs = {
  users: {
    title: '用户管理',
    actions: <CreateUserAction />,
    endpoint: '/admin/api/v1/users',
    responseKey: 'users',
    filters: [userFilter, emailFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <UserRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '用户ID' },
      { key: 'email', title: '邮箱' },
      { key: 'phone', title: '手机号' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'kyc_level', title: 'KYC等级' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  assets: {
    title: '资产管理',
    actions: <CreateAssetAction />,
    endpoint: '/admin/api/v1/assets',
    responseKey: 'assets',
    filters: [{ key: 'symbol', label: '资产符号' }, assetTypeFilter, statusSelectFilter, limitFilter],
    rowActions: (record, helpers) => <AssetRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '资产ID' },
      { key: 'symbol', title: '资产符号' },
      { key: 'name', title: '资产名称' },
      { key: 'precision_scale', title: '精度' },
      { key: 'asset_type', title: '资产类型', valueMap: assetTypeLabels },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  walletAccounts: {
    title: '钱包账户',
    endpoint: '/admin/api/v1/wallet/accounts',
    responseKey: 'accounts',
    filters: [userFilter, emailFilter, assetFilter, limitFilter],
    columns: [
      { key: 'id', title: '账户ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'available', title: '可用', type: 'amount' },
      { key: 'frozen', title: '冻结', type: 'amount' },
      { key: 'locked', title: '锁定', type: 'amount' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  walletLedger: {
    title: '钱包流水',
    endpoint: '/admin/api/v1/wallet/ledger',
    responseKey: 'ledger',
    filters: [userFilter, emailFilter, assetFilter, { key: 'change_type', label: '变动类型' }, { key: 'ref_type', label: '来源类型' }, limitFilter],
    columns: [
      { key: 'id', title: '流水ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'change_type', title: '变动类型' },
      { key: 'balance_type', title: '余额类型' },
      { key: 'amount', title: '金额', type: 'amount' },
      { key: 'balance_after', title: '变动后余额', type: 'amount' },
      { key: 'ref_type', title: '来源类型' },
      { key: 'ref_id', title: '来源ID' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  riskRules: {
    title: '风控规则',
    actions: <CreateRiskRuleAction />,
    endpoint: '/admin/api/v1/risk/rules',
    responseKey: 'rules',
    filters: [
      { key: 'rule_type', label: '规则类型' },
      { key: 'target_type', label: '对象类型' },
      { key: 'enabled', label: '启用' },
      limitFilter
    ],
    rowActions: (record, helpers) => <RiskRuleRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '规则ID' },
      { key: 'rule_type', title: '规则类型' },
      { key: 'target_type', title: '对象类型' },
      { key: 'target_id', title: '对象ID' },
      { key: 'enabled', title: '启用', type: 'status' },
      { key: 'created_by', title: '创建管理员' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  riskEvents: {
    title: '风控事件',
    endpoint: '/admin/api/v1/risk/events',
    responseKey: 'events',
    filters: [userFilter, emailFilter, { key: 'decision', label: '决策' }, { key: 'risk_level', label: '风险等级' }, limitFilter],
    columns: [
      { key: 'id', title: '事件ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'actor_type', title: '触发方' },
      { key: 'actor_id', title: '触发方ID' },
      { key: 'event_type', title: '事件类型' },
      { key: 'risk_level', title: '风险等级', type: 'status' },
      { key: 'decision', title: '决策', type: 'status' },
      { key: 'reason', title: '原因' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  agentCommissions: {
    title: '代理佣金',
    endpoint: '/admin/api/v1/agent-commissions',
    responseKey: 'commissions',
    filters: [userFilter, emailFilter, { key: 'agent_id', label: '代理ID' }, agentCommissionStatusFilter, limitFilter],
    rowActions: (record, helpers) => <AgentCommissionRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: 'ID' },
      { key: 'agent_id', title: '代理ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'source_type', title: '来源类型' },
      { key: 'source_id', title: '来源ID' },
      { key: 'source_amount', title: '来源金额', type: 'amount' },
      { key: 'commission_amount', title: '佣金金额', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  agentCommissionRules: {
    title: '佣金规则',
    actions: ({ reload }) => <CreateAgentCommissionRuleAction onCreated={reload} />,
    endpoint: '/admin/api/v1/agent-commission-rules',
    responseKey: 'rules',
    filters: [{ key: 'agent_id', label: '代理ID' }, agentCommissionRuleProductFilter, agentCommissionRuleStatusFilter, limitFilter],
    rowActions: (record, helpers) => <AgentCommissionRuleRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: 'ID' },
      { key: 'agent_id', title: '代理ID' },
      { key: 'product_type', title: '产品类型' },
      { key: 'commission_rate', title: '佣金比例', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  news: {
    title: '新闻中心',
    actions: ({ reload }) => <CreateAdminNewsAction onCreated={reload} />,
    endpoint: '/admin/api/v1/news',
    responseKey: 'news',
    filters: [
      { key: 'q', label: '关键词' },
      newsStatusFilter,
      newsCategoryFilter,
      { key: 'country_code', label: '国家' },
      { key: 'locale', label: '语言' },
      limitFilter
    ],
    rowActions: (record, helpers) => <AdminNewsRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '新闻ID' },
      { key: 'title', title: '标题' },
      { key: 'category', title: '分类', valueMap: newsCategoryLabels },
      { key: 'country_code', title: '国家' },
      { key: 'default_locale', title: '默认语言' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'published_at', title: '发布时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  marketPairs: {
    title: '交易对',
    actions: <CreateSpotPairAction />,
    endpoint: '/admin/api/v1/market-pairs',
    responseKey: 'pairs',
    filters: [marketPairSymbolFilter, marketPairStatusFilter, marketTypeFilter, limitFilter],
    rowActions: (record, helpers) => <MarketPairRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'base_asset', title: '基础资产' },
      { key: 'quote_asset', title: '计价资产' },
      { key: 'price_precision', title: '价格精度' },
      { key: 'qty_precision', title: '数量精度' },
      { key: 'min_order_value', title: '最小下单额', type: 'amount' },
      { key: 'symbol', title: '最新价格', render: (record) => <MarketPairLatestPrice symbol={record.symbol} /> },
      { key: 'market_type', title: '市场类型', valueMap: marketTypeLabels },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  spotOrders: {
    title: '现货订单',
    endpoint: '/admin/api/v1/spot/orders',
    responseKey: 'orders',
    filters: [userFilter, emailFilter, pairFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <SpotOrderRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '订单ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'side', title: '方向' },
      { key: 'order_type', title: '订单类型' },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'filled_quantity', title: '已成交数量', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  spotTrades: {
    title: '现货成交',
    endpoint: '/admin/api/v1/spot/trades',
    responseKey: 'trades',
    filters: [userFilter, emailFilter, pairFilter, limitFilter],
    columns: [
      { key: 'id', title: '成交ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'buy_order_id', title: '买单ID' },
      { key: 'sell_order_id', title: '卖单ID' },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'fee', title: '手续费', type: 'amount' },
      { key: 'created_at', title: '成交时间', type: 'timestamp' }
    ]
  },
  newCoinProjects: {
    title: '新币项目',
    actions: <CreateNewCoinProjectAction />,
    endpoint: '/admin/api/v1/new-coins',
    responseKey: 'projects',
    filters: [limitFilter],
    columns: [
      { key: 'id', title: '项目ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'symbol', title: '币种' },
      { key: 'lifecycle_status', title: '生命周期', type: 'status' },
      { key: 'total_supply', title: '总量', type: 'amount' },
      { key: 'issue_price', title: '发行价', type: 'amount' },
      { key: 'unlock_type', title: '解禁类型' },
      { key: 'listed_at', title: '上市时间', type: 'timestamp' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  newCoinSubscriptions: {
    title: '发行申购',
    endpoint: '/admin/api/v1/new-coins/subscriptions',
    responseKey: 'subscriptions',
    filters: [projectFilter, userFilter, emailFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '申购ID' },
      { key: 'project_id', title: '项目ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'quote_asset', title: '支付资产' },
      { key: 'quote_amount', title: '支付金额', type: 'amount' },
      { key: 'requested_quantity', title: '申购数量', type: 'amount' },
      { key: 'allocated_quantity', title: '获配数量', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  newCoinDistributions: {
    title: '派发记录',
    endpoint: '/admin/api/v1/new-coins/distributions',
    responseKey: 'distributions',
    filters: [projectFilter, userFilter, emailFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '派发ID' },
      { key: 'project_id', title: '项目ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'subscription_id', title: '申购ID' },
      { key: 'quantity', title: '派发数量', type: 'amount' },
      { key: 'lock_position_id', title: '锁仓ID' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  newCoinPurchases: {
    title: '上市认购',
    endpoint: '/admin/api/v1/new-coins/purchases',
    responseKey: 'purchases',
    filters: [projectFilter, userFilter, emailFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '订单ID' },
      { key: 'project_id', title: '项目ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'quote_amount', title: '支付金额', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  newCoinLockPositions: {
    title: '锁仓仓位',
    endpoint: '/admin/api/v1/new-coins/lock-positions',
    responseKey: 'lock_positions',
    filters: [userFilter, emailFilter, assetFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '仓位ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'unlock_type', title: '解禁类型' },
      { key: 'unlock_at', title: '解禁时间', type: 'timestamp' },
      { key: 'locked_amount', title: '锁定数量', type: 'amount' },
      { key: 'released_amount', title: '已释放', type: 'amount' },
      { key: 'remaining_amount', title: '剩余', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  newCoinUnlocks: {
    title: '解禁记录',
    endpoint: '/admin/api/v1/new-coins/unlocks',
    responseKey: 'unlocks',
    filters: [userFilter, emailFilter, assetFilter, statusFilter, { key: 'fee_paid_status', label: '矿工费状态' }, limitFilter],
    columns: [
      { key: 'id', title: '解禁ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'lock_position_id', title: '锁仓ID' },
      { key: 'unlock_quantity', title: '解禁数量', type: 'amount' },
      { key: 'unlock_fee_amount', title: '矿工费', type: 'amount' },
      { key: 'fee_paid_status', title: '矿工费状态', type: 'status' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  convertPairs: {
    title: '闪兑交易对',
    actions: <CreateConvertPairAction />,
    endpoint: '/admin/api/v1/convert/pairs',
    responseKey: 'pairs',
    filters: [limitFilter],
    rowActions: (record, helpers) => <ConvertPairRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '交易对ID' },
      { key: 'from_asset_id', title: '源资产' },
      { key: 'to_asset_id', title: '目标资产' },
      { key: 'pricing_mode', title: '定价模式' },
      { key: 'spread_rate', title: '价差率', type: 'amount' },
      { key: 'min_amount', title: '最小金额', type: 'amount' },
      { key: 'max_amount', title: '最大金额', type: 'amount' },
      { key: 'enabled', title: '启用', type: 'status' }
    ]
  },
  convertOrders: {
    title: '闪兑订单',
    endpoint: '/admin/api/v1/convert/orders',
    responseKey: 'orders',
    filters: [userFilter, emailFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <ConvertOrderRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '订单ID' },
      { key: 'quote_id', title: '报价ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'convert_pair_id', title: '交易对ID' },
      { key: 'from_amount', title: '源金额', type: 'amount' },
      { key: 'to_amount', title: '目标金额', type: 'amount' },
      { key: 'rate', title: '汇率', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  marketStrategies: {
    title: '行情策略',
    endpoint: '/admin/api/v1/market-strategies',
    responseKey: 'strategies',
    filters: [pairFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '策略ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'strategy_type', title: '策略类型' },
      { key: 'start_price', title: '起始价', type: 'amount' },
      { key: 'target_price', title: '目标价', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'run_status', title: '运行状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  marketStrategyActions: {
    title: '行情策略动作',
    actions: ({ reload }) => <CreateMarketStrategyAction onCreated={reload} />,
    endpoint: '/admin/api/v1/market-strategies',
    responseKey: 'strategies',
    filters: [pairFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <MarketStrategyRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '策略ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'strategy_type', title: '策略类型' },
      { key: 'start_price', title: '起始价', type: 'amount' },
      { key: 'target_price', title: '目标价', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'run_status', title: '运行状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  secondsProducts: {
    title: '秒合约产品',
    actions: <CreateSecondsPairAction />,
    endpoint: '/admin/api/v1/seconds-contracts/products',
    responseKey: 'products',
    filters: [limitFilter],
    rowActions: (record, helpers) => <SecondsProductRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '产品ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'stake_asset_symbol', title: '押注资产' },
      { key: 'duration_seconds', title: '周期秒数' },
      { key: 'payout_rate', title: '赔率', type: 'amount' },
      { key: 'min_stake', title: '最小押注', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  secondsOrders: {
    title: '秒合约订单',
    endpoint: '/admin/api/v1/seconds-contracts/orders',
    responseKey: 'orders',
    filters: [userFilter, emailFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <SecondsOrderRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '订单ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'product_id', title: '产品ID' },
      { key: 'direction', title: '方向' },
      { key: 'stake_amount', title: '押注金额', type: 'amount' },
      { key: 'entry_price', title: '入场价', type: 'amount' },
      { key: 'result', title: '结果', type: 'status' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'expires_at', title: '到期时间', type: 'timestamp' }
    ]
  },
  marginProducts: {
    title: '杠杆产品',
    actions: <CreateMarginPairAction />,
    endpoint: '/admin/api/v1/margin/products',
    responseKey: 'products',
    filters: [limitFilter],
    rowActions: (record, helpers) => <MarginProductRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '产品ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'margin_asset_symbol', title: '保证金资产' },
      { key: 'margin_mode', title: '保证金模式', valueMap: marginModeLabels },
      { key: 'leverage_levels', title: '杠杆档位', render: (record) => <MarginLeverageLevels levels={record.leverage_levels} /> },
      { key: 'max_leverage', title: '最大杠杆', type: 'amount' },
      { key: 'min_margin', title: '最小保证金', type: 'amount' },
      { key: 'maintenance_margin_rate', title: '维持保证金率', type: 'amount' },
      { key: 'hourly_interest_rate', title: '小时利率', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  marginPositions: {
    title: '杠杆仓位',
    endpoint: '/admin/api/v1/margin/positions',
    responseKey: 'positions',
    filters: [userFilter, emailFilter, pairFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <MarginPositionRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '仓位ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'product_id', title: '产品ID' },
      { key: 'direction', title: '方向' },
      { key: 'margin_amount', title: '保证金', type: 'amount' },
      { key: 'notional_amount', title: '名义金额', type: 'amount' },
      { key: 'borrowed_amount', title: '借款本金', type: 'amount' },
      { key: 'interest_amount', title: '累计利息', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  marginLiquidations: {
    title: '强平记录',
    endpoint: '/admin/api/v1/margin/liquidations',
    responseKey: 'liquidations',
    filters: [userFilter, emailFilter, pairFilter, { key: 'position_id', label: '仓位ID' }, limitFilter],
    rowActions: (record, helpers) => <MarginLiquidationRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '记录ID' },
      { key: 'position_id', title: '仓位ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'mark_price', title: '标记价', type: 'amount' },
      { key: 'equity', title: '权益', type: 'amount' },
      { key: 'interest_amount', title: '累计利息', type: 'amount' },
      { key: 'payout_amount', title: '返还金额', type: 'amount' },
      { key: 'reason', title: '原因' },
      { key: 'liquidated_at', title: '强平时间', type: 'timestamp' }
    ]
  },
  marginInterest: {
    title: '利息汇总',
    endpoint: '/admin/api/v1/margin/interest/summary',
    responseKey: 'summaries',
    filters: [userFilter, emailFilter, pairFilter, statusFilter, limitFilter],
    columns: [
      { key: 'margin_asset', title: '保证金资产' },
      { key: 'status', title: '仓位状态', type: 'status' },
      { key: 'position_count', title: '仓位数量' },
      { key: 'borrowed_amount', title: '借款合计', type: 'amount' },
      { key: 'interest_amount', title: '利息合计', type: 'amount' }
    ]
  },
  earnProducts: {
    title: '理财产品',
    actions: ({ reload }) => <CreateEarnProductAction onCreated={reload} />,
    endpoint: '/admin/api/v1/earn/products',
    responseKey: 'products',
    filters: [limitFilter],
    rowActions: (record, helpers) => <EarnProductRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '产品ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'name', title: '产品名称' },
      { key: 'category', title: '产品分类', valueMap: earnProductCategoryLabels },
      { key: 'term_days', title: '期限天数' },
      { key: 'apr_rate', title: '年化利率', type: 'amount' },
      { key: 'min_subscribe', title: '最小申购', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  earnSubscriptions: {
    title: '理财申购',
    endpoint: '/admin/api/v1/earn/subscriptions',
    responseKey: 'subscriptions',
    filters: [userFilter, emailFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <EarnSubscriptionRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '申购ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'product_id', title: '产品ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'amount', title: '申购金额', type: 'amount' },
      { key: 'apr_rate', title: '年化利率', type: 'amount' },
      { key: 'term_days', title: '期限天数' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'matures_at', title: '到期时间', type: 'timestamp' }
    ]
  },
  auditLogs: {
    title: '审计日志',
    endpoint: '/admin/api/v1/audit-logs',
    responseKey: 'logs',
    filters: [
      { key: 'admin_id', label: '管理员ID' },
      { key: 'action', label: '操作' },
      { key: 'target_type', label: '对象类型' },
      { key: 'target_id', label: '对象ID' },
      limitFilter
    ],
    columns: [
      { key: 'id', title: '日志ID' },
      { key: 'admin_id', title: '管理员ID' },
      { key: 'action', title: '操作' },
      { key: 'target_type', title: '对象类型' },
      { key: 'target_id', title: '对象ID' },
      { key: 'reason', title: '原因' },
      { key: 'ip', title: 'IP' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  }
} satisfies Record<string, ResourceConfig>;

export function ResourcePage({ config }: { config: ResourceConfig }) {
  return <AdminResourcePage<ApiRecord> {...config} />;
}
