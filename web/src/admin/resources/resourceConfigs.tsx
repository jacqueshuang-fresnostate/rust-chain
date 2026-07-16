import { useEffect, useMemo, useState } from 'react';

import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import {
  AgentCommissionRowActions,
  AgentCommissionRuleRowActions,
  AdminNewsRowActions,
  AssetRowActions,
  ConvertOrderRowActions,
  ConvertPairRowActions,
  CountryRowActions,
  CreateAgentCommissionRuleAction,
  CreateAdminNewsAction,
  CreateAssetAction,
  CreateConvertPairAction,
  CreateCountryAction,
  CreateDepositAddressPoolAction,
  CreateDepositNetworkConfigAction,
  CreateEarnCategoryAction,
  CreateEarnProductAction,
  CreateLoanProductAction,
  CreateMarginPairAction,
  CreateMarketStrategyAction,
  CreateNewCoinProjectAction,
  CreateRiskRuleAction,
  CreateUserAction,
  CreateSecondsPairAction,
  CreateSpotPairAction,
  DepositAddressPoolRowActions,
  DepositNetworkConfigRowActions,
  EarnCategoryRowActions,
  EarnProductRowActions,
  EarnSubscriptionRowActions,
  LoanOrderRowActions,
  LoanProductRowActions,
  MarginLiquidationRowActions,
  MarginPositionRowActions,
  MarginProductRowActions,
  MarketPairRowActions,
  MarketStrategyRowActions,
  QuickRechargeOrderRowActions,
  RiskRuleRowActions,
  SecondsOrderRowActions,
  SecondsProductRowActions,
  SpotOrderRowActions,
  UserRowActions
} from './ResourceCreateActions';
import { subscribeMarketTicker } from '../../api/marketTickerSocket';
import type { FilterField } from '../../shared/FilterBar';
import { AdminImageCell } from '../../shared/AdminImageUpload';
import { formatAdminNumber } from '../../shared/numberFormat';
import { formatBusinessOrderNo } from '../../shared/orderNo';
import type { ApiRecord } from '../../api/types';
import { PredictionMarketRowActions } from '../actions/PredictionMarketRowActions';

export type ResourceConfig = {
  actions?: React.ComponentProps<typeof AdminResourcePage<ApiRecord>>['actions'];
  columns: Array<AdminResourceColumn<ApiRecord>>;
  endpoint: string;
  filters?: FilterField[];
  responseKey: string;
  rowActions?: React.ComponentProps<typeof AdminResourcePage<ApiRecord>>['rowActions'];
  showJsonAction?: boolean;
  title: string;
  toolbarFilters?: FilterField[];
};

const limitFilter: FilterField = { key: 'limit', label: '数量限制' };
const statusFilter: FilterField = { key: 'status', label: '状态' };
const userFilter: FilterField = { key: 'user_id', label: '用户ID' };
const emailFilter: FilterField = { key: 'email', label: '邮箱' };
const pairFilter: FilterField = { key: 'pair_id', label: '交易对ID' };
const projectFilter: FilterField = { key: 'project_id', label: '项目ID' };
const assetFilter: FilterField = { key: 'asset_id', label: '资产ID' };

const orderNoColumn = (prefix: string, title = '订单号'): AdminResourceColumn<ApiRecord> => ({
  key: 'order_no',
  title,
  render: (record) => <span>{formatBusinessOrderNo(prefix, record)}</span>
});

const relatedOrderNoColumn = (key: string, prefix: string, title: string): AdminResourceColumn<ApiRecord> => ({
  key,
  title,
  render: (record) => <span>{formatBusinessOrderNo(prefix, { id: record[key], created_at: record.created_at })}</span>
});

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
const depositNetworkLabels = {
  eth: 'ETH',
  base: 'Base',
  tron: 'Tron',
  btc: 'BTC',
  solana: 'Solana'
};
const depositNetworkConfigStatusLabels = {
  active: '启用',
  disabled: '停用'
};
const depositAddressStatusLabels = {
  available: '可用',
  assigned: '已分配',
  disabled: '禁用'
};
const quickRechargeStatusLabels = {
  created: '已创建',
  pending: '待支付',
  paid: '已支付',
  failed: '失败',
  expired: '已过期'
};
const quickRechargeReturnTargetLabels = {
  pc_app: 'PC 应用端',
  mac_app: 'Mac 应用端',
  ios_app: 'iOS 端',
  android_app: 'Android 端',
  mobile_web: '手机网页端',
  desktop_web: '电脑网页端'
};
const spotOrderSideLabels = {
  buy: '买入',
  sell: '卖出'
};
const spotOrderTypeLabels = {
  limit: '限价单',
  market: '市价单'
};
const spotOrderStatusLabels = {
  pending: '待处理',
  open: '当前委托',
  partially_filled: '部分成交',
  filled: '已成交',
  cancelled: '已撤销',
  rejected: '已拒绝'
};
const spotOrderStatusOptions = Object.entries(spotOrderStatusLabels).map(([value, label]) => ({ label, value }));
const spotOrderPairFilter: FilterField = { key: 'pair_id', label: '交易对', type: 'select', optionsFromRows: true };
const spotOrderStatusFilter: FilterField = { key: 'status', label: '状态', type: 'select', options: spotOrderStatusOptions };
const spotOrderInternalFilter: FilterField = { key: 'include_internal', label: '显示机器人订单', type: 'switch' };
const robotDataFilter: FilterField = { key: 'include_internal', label: '显示机器人数据', type: 'switch' };
const earnEarlyRedeemFeeBasisLabels = {
  none: '不扣费',
  principal: '按本金比例扣除',
  profit: '按收益比例扣除'
};
const loanTypeLabels = {
  credit: '信用贷',
  collateralized: '抵押贷'
};
const loanInterestModeLabels = {
  full_term: '完整周期计息',
  actual_days: '按实际天数计息'
};
const loanOrderStatusLabels = {
  pending: '待审核',
  disbursed: '已放款',
  rejected: '已拒绝',
  cancelled: '已取消',
  repaid: '已还款'
};
const walletLedgerChangeTypeLabels = {
  deposit: '充值',
  admin_recharge: '后台充值',
  quick_recharge: '快速充值',
  convert_settlement: '闪兑结算',
  spot_freeze: '现货委托冻结',
  spot_unfreeze: '现货委托解冻',
  spot_fill: '现货成交',
  spot_trade_settlement: '现货成交结算',
  spot_price_improvement_release: '现货差价释放',
  seconds_contract_open: '秒合约开仓',
  seconds_contract_settle_win: '秒合约盈利结算',
  margin_position_open: '杠杆开仓',
  margin_position_close: '杠杆平仓',
  margin_position_liquidate: '杠杆强平',
  earn_subscribe: '理财申购',
  earn_redeem: '理财赎回',
  loan_collateral_freeze: '贷款抵押冻结',
  loan_collateral_release: '贷款抵押释放',
  loan_disbursement: '贷款放款',
  loan_repayment: '贷款还款',
  prediction_stake_freeze: '竞猜下注冻结',
  prediction_fee: '竞猜手续费',
  prediction_settle_win: '竞猜盈利结算',
  prediction_settle_loss: '竞猜亏损结算',
  prediction_payout: '竞猜派彩',
  prediction_stake_refund: '竞猜本金退款',
  prediction_fee_refund: '竞猜手续费退款',
  new_coin_subscription_payment: '新币申购支付',
  new_coin_subscription_lock: '新币申购锁仓',
  new_coin_purchase_payment: '新币购买支付',
  new_coin_purchase_lock: '新币购买锁仓',
  new_coin_distribution_lock: '新币派发锁仓',
  new_coin_unlock_release: '新币解禁释放',
  asset_lock: '资产锁定',
  agent_commission_payout: '代理佣金发放'
};
const walletLedgerBalanceTypeLabels = {
  available: '可用余额',
  frozen: '冻结余额',
  locked: '锁定余额'
};
const walletLedgerRefTypeLabels = {
  manual: '人工记录',
  deposit_record: '充值记录',
  admin_recharge: '后台充值',
  quick_recharge: '快速充值',
  convert_order: '闪兑订单',
  spot_order: '现货委托',
  spot_trade: '现货成交',
  seconds_contract_order: '秒合约订单',
  margin_position: '杠杆仓位',
  earn_subscription: '理财订单',
  loan_order: '贷款订单',
  prediction_order: '竞猜订单',
  new_coin_subscription: '新币申购',
  new_coin_purchase: '新币购买',
  new_coin_distribution: '新币派发',
  new_coin_unlock: '新币解禁',
  agent_commission: '代理佣金'
};
const depositNetworkFilter: FilterField = {
  key: 'network',
  label: '网络',
  type: 'select',
  options: Object.entries(depositNetworkLabels).map(([value, label]) => ({ label, value }))
};
const depositNetworkConfigStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: Object.entries(depositNetworkConfigStatusLabels).map(([value, label]) => ({ label, value }))
};
const depositAddressStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: Object.entries(depositAddressStatusLabels).map(([value, label]) => ({ label, value }))
};
const quickRechargeStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: Object.entries(quickRechargeStatusLabels).map(([value, label]) => ({ label, value }))
};
const walletLedgerAssetFilter: FilterField = { key: 'asset_id', label: '资产', optionLabelKey: 'asset_symbol', type: 'select', optionsFromRows: true };
const walletLedgerChangeTypeFilter: FilterField = {
  key: 'change_type',
  label: '变动类型',
  type: 'select',
  options: Object.entries(walletLedgerChangeTypeLabels).map(([value, label]) => ({ label, value }))
};
const walletLedgerRefTypeFilter: FilterField = {
  key: 'ref_type',
  label: '来源类型',
  type: 'select',
  options: Object.entries(walletLedgerRefTypeLabels).map(([value, label]) => ({ label, value }))
};
const loanTypeFilter: FilterField = {
  key: 'loan_type',
  label: '贷款类型',
  type: 'select',
  options: Object.entries(loanTypeLabels).map(([value, label]) => ({ label, value }))
};
const loanProductStatusFilter: FilterField = { key: 'status', label: '状态', type: 'select', options: statusOptions };
const loanOrderStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: Object.entries(loanOrderStatusLabels).map(([value, label]) => ({ label, value }))
};
const predictionDisplayStatusLabels = {
  active: '显示',
  hidden: '隐藏'
};
const predictionSettlementStatusLabels = {
  open: '开放下注',
  pending_confirmation: '待确认',
  settled: '已结算',
  refunded: '已退款'
};
const predictionOutcomeLabels = {
  yes: 'YES',
  no: 'NO',
  invalid: '无效'
};
const predictionOrderStatusLabels = {
  open: '持仓中',
  settled: '已结算',
  refunded: '已退款'
};
const predictionSettlementModeLabels = {
  manual_confirm: '人工确认',
  auto: '自动结算'
};
const predictionMarketDisplayStatusFilter: FilterField = {
  key: 'display_status',
  label: '显示状态',
  type: 'select',
  options: Object.entries(predictionDisplayStatusLabels).map(([value, label]) => ({ label, value }))
};
const predictionSettlementStatusFilter: FilterField = {
  key: 'settlement_status',
  label: '结算状态',
  type: 'select',
  options: Object.entries(predictionSettlementStatusLabels).map(([value, label]) => ({ label, value }))
};
const predictionOrderStatusFilter: FilterField = {
  key: 'status',
  label: '订单状态',
  type: 'select',
  options: Object.entries(predictionOrderStatusLabels).map(([value, label]) => ({ label, value }))
};
const countryStatusOptions = [
  { label: '启用', value: 'active' },
  { label: '停用', value: 'disabled' }
];
const countryRegistrationOptions = [
  { label: '启用', value: 'true' },
  { label: '停用', value: 'false' }
];
const agentCommissionStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: [
    { label: '待结算', value: 'pending' },
    { label: '已结算', value: 'settled' },
    { label: '已拒绝', value: 'rejected' }
  ]
};
const agentCommissionRuleProductFilter: FilterField = {
  key: 'product_type',
  label: '产品类型',
  type: 'select',
  options: [
    { label: '闪兑', value: 'convert' },
    { label: '竞猜', value: 'prediction' },
    { label: '现货', value: 'spot' },
    { label: '杠杆', value: 'margin' },
    { label: '秒合约', value: 'seconds_contract' }
  ]
};
const agentCommissionRuleStatusFilter: FilterField = {
  key: 'status',
  label: '状态',
  type: 'select',
  options: [
    { label: '启用', value: 'active' },
    { label: '禁用', value: 'disabled' }
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

function MarginModeList({ modes }: { modes: unknown }) {
  const normalizedModes = Array.isArray(modes) ? modes : typeof modes === 'string' ? [modes] : [];
  const labels = normalizedModes
    .map((mode) => (typeof mode === 'string' ? marginModeLabels[mode as keyof typeof marginModeLabels] ?? mode : null))
    .filter((mode): mode is string => Boolean(mode));

  return <span>{labels.length > 0 ? labels.join(' / ') : '-'}</span>;
}

function DepositAssetSymbolList({ fallback, value }: { fallback: unknown; value: unknown }) {
  const symbols = Array.isArray(value)
    ? value.map((item) => (typeof item === 'string' ? item.trim() : '')).filter((item) => item.length > 0)
    : [];
  const fallbackSymbol = typeof fallback === 'string' ? fallback.trim() : '';

  return <span>{symbols.length > 0 ? symbols.join(' / ') : fallbackSymbol || '任意资产'}</span>;
}

function WithdrawFeeTiersCell({ value }: { value: unknown }) {
  const tiers = Array.isArray(value) ? value : [];
  const labels = tiers
    .map((item) => {
      if (!item || typeof item !== 'object') return null;
      const tier = item as Record<string, unknown>;
      const minAmount = typeof tier.min_amount === 'number' || typeof tier.min_amount === 'string' ? String(tier.min_amount) : '';
      const maxAmount = typeof tier.max_amount === 'number' || typeof tier.max_amount === 'string' ? String(tier.max_amount) : '';
      const feeRate = typeof tier.fee_rate_percent === 'number' || typeof tier.fee_rate_percent === 'string' ? String(tier.fee_rate_percent) : '';
      if (!minAmount || !feeRate) return null;
      return maxAmount ? `${minAmount} - ${maxAmount}: ${feeRate}%` : `${minAmount}以上: ${feeRate}%`;
    })
    .filter((label): label is string => Boolean(label));

  return <span>{labels.length > 0 ? labels.join(' / ') : '-'}</span>;
}

function adminNumberText(value: unknown): string {
  return typeof value === 'number' || typeof value === 'string' ? formatAdminNumber(value) ?? '-' : '-';
}

function adminRawText(value: unknown): string {
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '-';
}

function SecondsProductCyclesText({ record }: { record: ApiRecord }) {
  const cycles = Array.isArray(record.cycles) ? record.cycles : [];
  const source = cycles.length
    ? cycles
    : [
        {
          duration_seconds: record.duration_seconds,
          payout_rate: record.payout_rate,
          min_stake: record.min_stake,
          max_stake: record.max_stake
        }
      ];
  const text = source
    .map((cycle) => {
      if (!cycle || typeof cycle !== 'object') {
        return '';
      }
      const item = cycle as ApiRecord;
      const maxStake = item.max_stake;
      const maxText = maxStake === null || maxStake === undefined || maxStake === '' ? '无上限' : adminNumberText(maxStake);
      return `${adminRawText(item.duration_seconds)}s / 赔率 ${adminNumberText(item.payout_rate)} / ${adminNumberText(item.min_stake)}-${maxText}`;
    })
    .filter(Boolean)
    .join('；');

  return <span>{text || '-'}</span>;
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

function InlineAmount({ unit, value }: { unit: unknown; value: unknown }) {
  const amount = typeof value === 'number' || typeof value === 'string' ? formatAdminNumber(value) : null;
  const suffix = typeof unit === 'string' && unit.trim() ? ` ${unit.toUpperCase()}` : '';
  return <span>{amount ? `${amount}${suffix}` : '-'}</span>;
}

function SupportedLocales({ value }: { value: unknown }) {
  const locales = Array.isArray(value) ? value.filter((locale): locale is string => typeof locale === 'string') : [];
  return <span>{locales.length > 0 ? locales.join(' / ') : '-'}</span>;
}

function EarnCategoryLocales({ value }: { value: unknown }) {
  const items = value && typeof value === 'object' && 'items' in value && Array.isArray((value as { items?: unknown }).items) ? (value as { items: unknown[] }).items : [];
  const locales = items
    .map((item) => {
      if (!item || typeof item !== 'object') {
        return null;
      }
      const record = item as Record<string, unknown>;
      const locale = typeof record.locale === 'string' ? record.locale : '';
      const title = typeof record.title === 'string' ? record.title : '';
      return locale && title ? `${locale}: ${title}` : locale || title || null;
    })
    .filter((item): item is string => Boolean(item));

  return <span>{locales.length > 0 ? locales.join(' / ') : '-'}</span>;
}

export const resourceConfigs = {
  users: {
    title: '用户管理',
    actions: ({ reload }) => <CreateUserAction onCreated={reload} />,
    endpoint: '/admin/api/v1/users',
    responseKey: 'users',
    filters: [userFilter, emailFilter, statusFilter, limitFilter],
    rowActions: (record, helpers) => <UserRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    toolbarFilters: [robotDataFilter],
    columns: [
      { key: 'id', title: '用户ID' },
      { key: 'email', title: '邮箱' },
      { key: 'phone', title: '手机号' },
      { key: 'invite_code', title: '邀请码' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'kyc_level', title: 'KYC等级' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  assets: {
    title: '资产管理',
    actions: ({ reload }) => <CreateAssetAction onCreated={reload} />,
    endpoint: '/admin/api/v1/assets',
    responseKey: 'assets',
    filters: [{ key: 'symbol', label: '资产符号' }, assetTypeFilter, statusSelectFilter, limitFilter],
    rowActions: (record, helpers) => <AssetRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '资产ID' },
      { key: 'symbol', title: '资产符号' },
      { key: 'logo_url', title: 'Logo', render: (record) => <AdminImageCell alt="资产 Logo" value={record.logo_url} /> },
      { key: 'name', title: '资产名称' },
      { key: 'precision_scale', title: '精度' },
      { key: 'asset_type', title: '资产类型', valueMap: assetTypeLabels },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'deposit_enabled', title: '支持充值', type: 'status' },
      { key: 'withdraw_enabled', title: '支持提现', type: 'status' },
      { key: 'min_deposit_amount', title: '最小充值数量', type: 'amount' },
      { key: 'deposit_fee', title: '充值手续费', type: 'amount' },
      { key: 'withdraw_fee', title: '提现手续费', type: 'amount' },
      { key: 'withdraw_fee_tiers', title: '提现梯度手续费', render: (record) => <WithdrawFeeTiersCell value={record.withdraw_fee_tiers} /> },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  walletAccounts: {
    title: '钱包账户',
    endpoint: '/admin/api/v1/wallet/accounts',
    responseKey: 'accounts',
    filters: [userFilter, emailFilter, assetFilter, limitFilter],
    toolbarFilters: [robotDataFilter],
    columns: [
      { key: 'user_email', title: '用户邮箱' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'available', title: '可用', type: 'amount' },
      { key: 'frozen', title: '冻结', type: 'amount' },
      { key: 'locked', title: '锁定', type: 'amount' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  depositNetworkConfigs: {
    title: '充值网络配置',
    actions: ({ reload }) => <CreateDepositNetworkConfigAction onCreated={reload} />,
    endpoint: '/admin/api/v1/deposit-network-configs',
    responseKey: 'configs',
    filters: [depositNetworkFilter, { key: 'address_group_code', label: '地址集合编号' }, depositNetworkConfigStatusFilter, { key: 'asset_symbol', label: '资产符号' }, limitFilter],
    rowActions: (record, helpers) => <DepositNetworkConfigRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'network', title: '网络', valueMap: depositNetworkLabels },
      { key: 'display_name', title: '显示名称' },
      { key: 'address_group_code', title: '地址集合编号' },
      { key: 'address_group_name', title: '地址集合名称' },
      { key: 'asset_symbols', title: '支持充值币种', render: (record) => <DepositAssetSymbolList fallback={null} value={record.asset_symbols} /> },
      { key: 'status', title: '状态', valueMap: depositNetworkConfigStatusLabels },
      { key: 'sort_order', title: '排序' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  depositAddressPool: {
    title: '充值地址池',
    actions: ({ reload }) => <CreateDepositAddressPoolAction onCreated={reload} />,
    endpoint: '/admin/api/v1/deposit-address-pool',
    responseKey: 'addresses',
    filters: [depositNetworkFilter, { key: 'address_group_code', label: '地址集合编号' }, depositAddressStatusFilter, { key: 'asset_symbol', label: '资产符号' }, emailFilter, { key: 'address', label: '充值地址' }, limitFilter],
    rowActions: (record, helpers) => <DepositAddressPoolRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'network', title: '网络', valueMap: depositNetworkLabels },
      { key: 'address_group_code', title: '地址集合编号' },
      { key: 'address', title: '充值地址' },
      { key: 'asset_symbols', title: '限定资产', render: (record) => <DepositAssetSymbolList fallback={record.asset_symbol} value={record.asset_symbols} /> },
      { key: 'status', title: '状态', valueMap: depositAddressStatusLabels },
      { key: 'assigned_user_email', title: '绑定用户邮箱' },
      { key: 'assigned_asset_symbol', title: '绑定资产' },
      { key: 'assigned_at', title: '绑定时间', type: 'timestamp' },
      { key: 'memo', title: 'Memo / Tag' },
      { key: 'remark', title: '备注' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  quickRechargeOrders: {
    title: '快速充值订单',
    endpoint: '/admin/api/v1/quick-recharge/orders',
    responseKey: 'orders',
    filters: [emailFilter, quickRechargeStatusFilter, { key: 'order_id', label: '平台订单号' }, { key: 'provider_trade_id', label: 'GMPay 交易号' }, limitFilter],
    rowActions: (record, helpers) => <QuickRechargeOrderRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'order_id', title: '平台订单号' },
      { key: 'user_email', title: '用户邮箱' },
      { key: 'asset_symbol', title: '到账资产' },
      { key: 'fiat_amount', title: '充值金额', render: (record) => <InlineAmount unit={record.currency} value={record.fiat_amount} /> },
      { key: 'actual_amount', title: '到账数量', render: (record) => <InlineAmount unit={record.token} value={record.actual_amount} /> },
      { key: 'network', title: '网络' },
      { key: 'status', title: '状态', valueMap: quickRechargeStatusLabels },
      { key: 'return_target', title: '回跳端', valueMap: quickRechargeReturnTargetLabels },
      { key: 'redirect_url', title: '回跳地址' },
      { key: 'provider_trade_id', title: 'GMPay 交易号' },
      { key: 'block_transaction_id', title: '链上交易哈希' },
      { key: 'paid_at', title: '入账时间', type: 'timestamp' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  walletLedger: {
    title: '钱包流水',
    endpoint: '/admin/api/v1/wallet/ledger',
    responseKey: 'ledger',
    filters: [userFilter, emailFilter, walletLedgerAssetFilter, walletLedgerChangeTypeFilter, walletLedgerRefTypeFilter, limitFilter],
    toolbarFilters: [robotDataFilter],
    columns: [
      { key: 'id', title: '流水ID' },
      { key: 'user_email', title: '用户邮箱' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'change_type', title: '变动类型', valueMap: walletLedgerChangeTypeLabels },
      { key: 'balance_type', title: '余额类型', valueMap: walletLedgerBalanceTypeLabels },
      { key: 'amount', title: '金额', type: 'amount' },
      { key: 'balance_after', title: '变动后余额', type: 'amount' },
      { key: 'ref_type', title: '来源类型', valueMap: walletLedgerRefTypeLabels },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  loanProducts: {
    title: '贷款产品',
    actions: ({ reload }) => <CreateLoanProductAction onCreated={reload} />,
    endpoint: '/admin/api/v1/loan/products',
    responseKey: 'products',
    filters: [loanTypeFilter, loanProductStatusFilter, limitFilter],
    rowActions: (record, helpers) => <LoanProductRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'name', title: '默认产品名' },
      { key: 'name_json', title: '多语言名称', render: (record) => <EarnCategoryLocales value={record.name_json} /> },
      { key: 'loan_type', title: '贷款类型', valueMap: loanTypeLabels },
      { key: 'asset_symbol', title: '放款资产' },
      { key: 'term_days', title: '期限天数' },
      { key: 'interest_rate', title: '期限利率', type: 'amount' },
      { key: 'interest_calculation_mode', title: '提前还款计息', valueMap: loanInterestModeLabels },
      { key: 'min_kyc_level', title: '最低KYC等级' },
      { key: 'min_amount', title: '最小借款金额', type: 'amount' },
      { key: 'max_amount', title: '最大借款金额', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  loanOrders: {
    title: '贷款订单',
    endpoint: '/admin/api/v1/loan/orders',
    responseKey: 'orders',
    filters: [userFilter, emailFilter, { key: 'product_id', label: '贷款产品ID' }, loanTypeFilter, loanOrderStatusFilter, limitFilter],
    rowActions: (record, helpers) => <LoanOrderRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      orderNoColumn('LN'),
      { key: 'user_email', title: '用户邮箱' },
      { key: 'product_name', title: '贷款产品' },
      { key: 'loan_type', title: '贷款类型', valueMap: loanTypeLabels },
      { key: 'asset_symbol', title: '放款资产' },
      { key: 'amount', title: '借款本金', type: 'amount' },
      { key: 'interest_rate', title: '期限利率', type: 'amount' },
      { key: 'interest_calculation_mode', title: '计息方式', valueMap: loanInterestModeLabels },
      { key: 'term_days', title: '期限天数' },
      { key: 'collateral_asset_symbol', title: '抵押资产' },
      { key: 'collateral_amount', title: '抵押数量', type: 'amount' },
      { key: 'status', title: '状态', valueMap: loanOrderStatusLabels },
      { key: 'interest_amount', title: '实际利息', type: 'amount' },
      { key: 'repayment_amount', title: '还款总额', type: 'amount' },
      { key: 'due_at', title: '到期时间', type: 'timestamp' },
      { key: 'created_at', title: '申请时间', type: 'timestamp' }
    ]
  },
  predictionAssetConfigs: {
    title: '竞猜下注资产',
    endpoint: '/admin/api/v1/prediction/asset-configs',
    responseKey: 'configs',
    filters: [limitFilter],
    showJsonAction: false,
    columns: [
      { key: 'asset_symbol', title: '资产' },
      { key: 'enabled', title: '允许下注', type: 'status' },
      { key: 'max_payout_amount', title: '默认最大赔付', type: 'amount' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  predictionMarkets: {
    title: '竞猜市场',
    endpoint: '/admin/api/v1/prediction/markets',
    responseKey: 'markets',
    filters: [{ key: 'keyword', label: '关键词' }, predictionMarketDisplayStatusFilter, predictionSettlementStatusFilter, limitFilter],
    rowActions: (record, helpers) => <PredictionMarketRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'title', title: '市场标题' },
      { key: 'category', title: '分类' },
      { key: 'yes_price', title: 'YES 概率', type: 'amount' },
      { key: 'no_price', title: 'NO 概率', type: 'amount' },
      { key: 'volume', title: '成交量', type: 'amount' },
      { key: 'display_status', title: '显示状态', valueMap: predictionDisplayStatusLabels },
      { key: 'settlement_status', title: '结算状态', valueMap: predictionSettlementStatusLabels },
      { key: 'external_resolution', title: '外部结果', valueMap: predictionOutcomeLabels },
      { key: 'local_resolution', title: '本地结果', valueMap: predictionOutcomeLabels },
      { key: 'settlement_mode_override', title: '结算覆盖', valueMap: predictionSettlementModeLabels },
      { key: 'fee_rate_override', title: '手续费覆盖', type: 'amount' },
      { key: 'last_synced_at', title: '同步时间', type: 'timestamp' }
    ]
  },
  predictionOrders: {
    title: '竞猜订单',
    endpoint: '/admin/api/v1/prediction/orders',
    responseKey: 'orders',
    filters: [emailFilter, { key: 'market_id', label: '市场ID' }, predictionOrderStatusFilter, limitFilter],
    columns: [
      orderNoColumn('PM'),
      { key: 'user_email', title: '用户邮箱' },
      { key: 'market_title', title: '市场' },
      { key: 'outcome', title: '方向', valueMap: predictionOutcomeLabels },
      { key: 'asset_symbol', title: '下注资产' },
      { key: 'stake_amount', title: '下注金额', type: 'amount' },
      { key: 'fee_amount', title: '手续费', type: 'amount' },
      { key: 'accepted_price', title: '成交概率', type: 'amount' },
      { key: 'shares', title: '份额', type: 'amount' },
      { key: 'theoretical_payout', title: '理论赔付', type: 'amount' },
      { key: 'effective_payout_cap', title: '赔付封顶', type: 'amount' },
      { key: 'status', title: '状态', valueMap: predictionOrderStatusLabels },
      { key: 'result', title: '结果', valueMap: predictionOutcomeLabels },
      { key: 'payout_amount', title: '派彩金额', type: 'amount' },
      { key: 'refund_amount', title: '退款本金', type: 'amount' },
      { key: 'fee_refund_amount', title: '退回手续费', type: 'amount' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'settled_at', title: '结算时间', type: 'timestamp' }
    ]
  },
  predictionSyncLogs: {
    title: '竞猜同步日志',
    endpoint: '/admin/api/v1/prediction/sync/logs',
    responseKey: 'logs',
    filters: [limitFilter],
    columns: [
      { key: 'trigger_type', title: '触发方式' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'imported_count', title: '新增数量' },
      { key: 'updated_count', title: '更新数量' },
      { key: 'error_message', title: '错误信息' },
      { key: 'started_at', title: '开始时间', type: 'timestamp' },
      { key: 'finished_at', title: '结束时间', type: 'timestamp' }
    ]
  },
  riskRules: {
    title: '风控规则',
    actions: ({ reload }) => <CreateRiskRuleAction onCreated={reload} />,
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
      { key: 'payout_asset_id', title: '结算资产ID' },
      { key: 'commission_rate', title: '实际差额比例', type: 'amount' },
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
      { key: 'commission_rate', title: '累计返佣比例', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  countries: {
    title: '国家配置',
    actions: ({ reload }) => <CreateCountryAction onCreated={reload} />,
    endpoint: '/admin/api/v1/countries',
    responseKey: 'countries',
    filters: [
      { key: 'country_code', label: '国家代码' },
      { key: 'status', label: '状态', type: 'select', options: countryStatusOptions },
      { key: 'registration_enabled', label: '开放注册', type: 'select', options: countryRegistrationOptions },
      limitFilter
    ],
    rowActions: (record, helpers) => <CountryRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '国家配置ID' },
      { key: 'country_code', title: '国家代码' },
      { key: 'country_name', title: '国家名称' },
      { key: 'remark', title: '备注（中文名称）' },
      { key: 'default_locale', title: '默认语言' },
      { key: 'supported_locales', title: '支持语言', render: (record) => <SupportedLocales value={record.supported_locales} /> },
      { key: 'registration_enabled', title: '开放注册', type: 'status' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'sort_order', title: '排序' },
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
      { key: 'banner_url', title: 'Banner', render: (record) => <AdminImageCell alt="新闻 Banner" value={record.banner_url} /> },
      { key: 'small_logo_url', title: '小 Logo', render: (record) => <AdminImageCell alt="新闻小 Logo" value={record.small_logo_url} /> },
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
    actions: ({ reload }) => <CreateSpotPairAction onCreated={reload} />,
    endpoint: '/admin/api/v1/market-pairs',
    responseKey: 'pairs',
    filters: [marketPairSymbolFilter, marketPairStatusFilter, marketTypeFilter, limitFilter],
    rowActions: (record, helpers) => <MarketPairRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'id', title: '交易对ID' },
      { key: 'symbol', title: '交易对' },
      { key: 'logo_url', title: 'Logo', render: (record) => <AdminImageCell alt="交易对 Logo" value={record.logo_url} /> },
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
    filters: [userFilter, emailFilter, spotOrderPairFilter, spotOrderStatusFilter, limitFilter],
    rowActions: (record, helpers) => <SpotOrderRowActions helpers={helpers} record={record} />,
    toolbarFilters: [spotOrderInternalFilter],
    columns: [
      orderNoColumn('SP'),
      { key: 'user_email', title: '用户邮箱' },
      { key: 'pair_id', title: '交易对' },
      { key: 'side', title: '方向', valueMap: spotOrderSideLabels },
      { key: 'order_type', title: '订单类型', valueMap: spotOrderTypeLabels },
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'average_price', title: '成交价', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'filled_quantity', title: '已成交数量', type: 'amount' },
      { key: 'status', title: '状态', valueMap: spotOrderStatusLabels }
    ]
  },
  spotTrades: {
    title: '现货成交',
    endpoint: '/admin/api/v1/spot/trades',
    responseKey: 'trades',
    filters: [userFilter, emailFilter, pairFilter, limitFilter],
    toolbarFilters: [robotDataFilter],
    columns: [
      { key: 'id', title: '成交ID' },
      { key: 'pair_id', title: '交易对ID' },
      relatedOrderNoColumn('buy_order_id', 'SP', '买单号'),
      relatedOrderNoColumn('sell_order_id', 'SP', '卖单号'),
      { key: 'price', title: '价格', type: 'amount' },
      { key: 'quantity', title: '数量', type: 'amount' },
      { key: 'fee', title: '手续费', type: 'amount' },
      { key: 'created_at', title: '成交时间', type: 'timestamp' }
    ]
  },
  newCoinProjects: {
    title: '新币项目',
    actions: ({ reload }) => <CreateNewCoinProjectAction onCreated={reload} />,
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
      orderNoColumn('NC', '申购号'),
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
      relatedOrderNoColumn('subscription_id', 'NC', '申购号'),
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
      orderNoColumn('NP'),
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
    actions: ({ reload }) => <CreateConvertPairAction onCreated={reload} />,
    endpoint: '/admin/api/v1/convert/pairs',
    responseKey: 'pairs',
    filters: [limitFilter],
    rowActions: (record, helpers) => <ConvertPairRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'id', title: '交易对ID' },
      { key: 'from_asset_symbol', title: '源资产' },
      { key: 'to_asset_symbol', title: '目标资产' },
      { key: 'pricing_mode', title: '定价模式' },
      { key: 'spread_rate', title: '价差率', type: 'amount' },
      { key: 'fee_rate', title: '手续费率', type: 'amount' },
      { key: 'min_amount', title: '源资产最小金额', type: 'amount' },
      { key: 'max_amount', title: '源资产最大金额', type: 'amount' },
      { key: 'target_min_amount', title: '目标资产最小金额', type: 'amount' },
      { key: 'target_max_amount', title: '目标资产最大金额', type: 'amount' },
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
      orderNoColumn('CV'),
      { key: 'user_email', title: '用户邮箱' },
      { key: 'from_asset_symbol', title: '源资产' },
      { key: 'to_asset_symbol', title: '目标资产' },
      { key: 'from_amount', title: '源金额', type: 'amount' },
      { key: 'to_amount', title: '目标金额', type: 'amount' },
      { key: 'rate', title: '汇率', type: 'amount' },
      { key: 'fee_rate', title: '手续费率', type: 'amount' },
      { key: 'fee_amount', title: '手续费金额', type: 'amount' },
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
    actions: ({ reload }) => <CreateSecondsPairAction onCreated={reload} />,
    endpoint: '/admin/api/v1/seconds-contracts/products',
    responseKey: 'products',
    filters: [limitFilter],
    rowActions: (record, helpers) => <SecondsProductRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'symbol', title: '交易对' },
      { key: 'logo_url', title: 'Logo', render: (record) => <AdminImageCell alt="秒合约交易对 Logo" value={record.logo_url} /> },
      { key: 'stake_asset_symbol', title: '押注资产' },
      { key: 'cycles', title: '周期 / 赔率 / 押注范围', render: (record) => <SecondsProductCyclesText record={record} /> },
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
      orderNoColumn('SC'),
      { key: 'email', title: '用户邮箱' },
      { key: 'symbol', title: '交易对' },
      { key: 'direction', title: '方向' },
      { key: 'stake_amount', title: '押注金额', type: 'amount' },
      { key: 'entry_price', title: '入场价', type: 'amount' },
      { key: 'settlement_price', title: '结算价格', type: 'amount' },
      { key: 'result', title: '结果', type: 'status' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'expires_at', title: '到期时间', type: 'timestamp' }
    ]
  },
  marginProducts: {
    title: '杠杆产品',
    actions: ({ reload }) => <CreateMarginPairAction onCreated={reload} />,
    endpoint: '/admin/api/v1/margin/products',
    responseKey: 'products',
    filters: [limitFilter],
    rowActions: (record, helpers) => <MarginProductRowActions helpers={helpers} record={record} />,
    columns: [
      { key: 'symbol', title: '交易对' },
      { key: 'logo_url', title: 'Logo', render: (record) => <AdminImageCell alt="杠杆交易对 Logo" value={record.logo_url} /> },
      { key: 'margin_asset_symbol', title: '保证金资产' },
      { key: 'margin_modes', title: '支持保证金模式', valueMap: marginModeLabels, render: (record) => <MarginModeList modes={record.margin_modes ?? record.margin_mode} /> },
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
  earnCategories: {
    title: '理财分类',
    actions: ({ reload }) => <CreateEarnCategoryAction onCreated={reload} />,
    endpoint: '/admin/api/v1/earn/categories',
    responseKey: 'categories',
    filters: [statusSelectFilter, limitFilter],
    rowActions: (record, helpers) => <EarnCategoryRowActions helpers={helpers} record={record} />,
    showJsonAction: false,
    columns: [
      { key: 'code', title: '分类代码' },
      { key: 'default_name', title: '默认栏目名' },
      { key: 'name_json', title: '多语言名称', render: (record) => <EarnCategoryLocales value={record.name_json} /> },
      { key: 'sort_order', title: '排序值' },
      { key: 'status', title: '状态', type: 'status' }
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
      { key: 'banner_url', title: 'Banner', render: (record) => <AdminImageCell alt="理财 Banner" value={record.banner_url} /> },
      { key: 'small_logo_url', title: '小 Logo', render: (record) => <AdminImageCell alt="理财小 Logo" value={record.small_logo_url} /> },
      { key: 'category_name', title: '产品分类' },
      { key: 'category', title: '分类代码' },
      { key: 'term_days', title: '期限天数' },
      { key: 'apr_rate', title: '年化利率', type: 'amount' },
      { key: 'redemption_fee_rate', title: '赎回手续费率', type: 'amount' },
      { key: 'maturity_profit_fee_rate', title: '到期收益手续费率', type: 'amount' },
      { key: 'early_redeem_fee_basis', title: '提前赎回扣费基准', valueMap: earnEarlyRedeemFeeBasisLabels },
      { key: 'early_redeem_fee_rate', title: '提前赎回扣费率', type: 'amount' },
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
      orderNoColumn('EA', '申购号'),
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
