import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import type { FilterField } from '../../shared/FilterBar';
import type { ApiRecord } from '../../api/types';

export type ResourceConfig = {
  columns: Array<AdminResourceColumn<ApiRecord>>;
  endpoint: string;
  filters?: FilterField[];
  responseKey: string;
  title: string;
};

const limitFilter: FilterField = { key: 'limit', label: '数量限制' };
const statusFilter: FilterField = { key: 'status', label: '状态' };
const userFilter: FilterField = { key: 'user_id', label: '用户ID' };
const pairFilter: FilterField = { key: 'pair_id', label: '交易对ID' };
const projectFilter: FilterField = { key: 'project_id', label: '项目ID' };
const assetFilter: FilterField = { key: 'asset_id', label: '资产ID' };

export const resourceConfigs = {
  users: {
    title: '用户管理',
    endpoint: '/admin/api/v1/users',
    responseKey: 'users',
    filters: [userFilter, statusFilter, limitFilter],
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
  walletAccounts: {
    title: '钱包账户',
    endpoint: '/admin/api/v1/wallet/accounts',
    responseKey: 'accounts',
    filters: [userFilter, assetFilter, limitFilter],
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
    filters: [userFilter, assetFilter, { key: 'change_type', label: '变动类型' }, { key: 'ref_type', label: '来源类型' }, limitFilter],
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
    endpoint: '/admin/api/v1/risk/rules',
    responseKey: 'rules',
    filters: [
      { key: 'rule_type', label: '规则类型' },
      { key: 'target_type', label: '对象类型' },
      { key: 'enabled', label: '启用' },
      limitFilter
    ],
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
    filters: [userFilter, { key: 'decision', label: '决策' }, { key: 'risk_level', label: '风险等级' }, limitFilter],
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
    filters: [userFilter, { key: 'agent_id', label: '代理ID' }, statusFilter, limitFilter],
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
  marketPairs: {
    title: '交易对',
    endpoint: '/admin/api/v1/market-pairs',
    responseKey: 'pairs',
    filters: [{ key: 'symbol', label: '交易对' }, statusFilter, { key: 'market_type', label: '市场类型' }, limitFilter],
    columns: [
      { key: 'id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'base_asset', title: '基础资产' },
      { key: 'quote_asset', title: '计价资产' },
      { key: 'price_precision', title: '价格精度' },
      { key: 'qty_precision', title: '数量精度' },
      { key: 'min_order_value', title: '最小下单额', type: 'amount' },
      { key: 'market_type', title: '市场类型' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  spotOrders: {
    title: '现货订单',
    endpoint: '/admin/api/v1/spot/orders',
    responseKey: 'orders',
    filters: [userFilter, pairFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '订单ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'side', title: '方向' },
      { key: 'order_type', title: '订单类型' },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' }
    ]
  },
  spotTrades: {
    title: '现货成交',
    endpoint: '/admin/api/v1/spot/trades',
    responseKey: 'trades',
    filters: [userFilter, pairFilter, limitFilter],
    columns: [
      { key: 'id', title: '成交ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'buy_order_id', title: '买单ID' },
      { key: 'sell_order_id', title: '卖单ID' },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'created_at', title: '成交时间', type: 'timestamp' }
    ]
  },
  newCoinProjects: {
    title: '新币项目',
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
    filters: [projectFilter, userFilter, statusFilter, limitFilter],
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
    filters: [projectFilter, userFilter, statusFilter, limitFilter],
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
    filters: [projectFilter, userFilter, statusFilter, limitFilter],
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
    filters: [userFilter, assetFilter, statusFilter, limitFilter],
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
    filters: [userFilter, assetFilter, statusFilter, { key: 'fee_paid_status', label: '矿工费状态' }, limitFilter],
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
    endpoint: '/admin/api/v1/convert/pairs',
    responseKey: 'pairs',
    filters: [limitFilter],
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
    filters: [userFilter, statusFilter, limitFilter],
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
  secondsProducts: {
    title: '秒合约产品',
    endpoint: '/admin/api/v1/seconds-contracts/products',
    responseKey: 'products',
    filters: [limitFilter],
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
    filters: [userFilter, statusFilter, limitFilter],
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
    endpoint: '/admin/api/v1/margin/products',
    responseKey: 'products',
    filters: [limitFilter],
    columns: [
      { key: 'id', title: '产品ID' },
      { key: 'pair_id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'margin_asset_symbol', title: '保证金资产' },
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
    filters: [userFilter, pairFilter, statusFilter, limitFilter],
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
    filters: [userFilter, pairFilter, { key: 'position_id', label: '仓位ID' }, limitFilter],
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
    filters: [userFilter, pairFilter, statusFilter, limitFilter],
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
    endpoint: '/admin/api/v1/earn/products',
    responseKey: 'products',
    filters: [limitFilter],
    columns: [
      { key: 'id', title: '产品ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'name', title: '产品名称' },
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
    filters: [userFilter, statusFilter, limitFilter],
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
