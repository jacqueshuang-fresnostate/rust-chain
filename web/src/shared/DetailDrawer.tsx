import { SideSheet, Table, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';

import type { ApiRecord } from '../api/types';
import { formatAdminBetContent, isAdminBetContentField } from './betContentFormat';
import { formatAdminDisplayValue, formatAdminNumber } from './numberFormat';
import { formatBusinessOrderNo } from './orderNo';
import { containedTableScroll, containedTableStyle } from './tableLayout';
import { formatAdminTimestamp } from './TimestampText';

const { Text } = Typography;

type DetailFieldType = 'amount' | 'json' | 'status' | 'text' | 'timestamp';

type DetailDrawerFieldMeta = {
  assets?: Record<string, string | undefined>;
  labels?: Record<string, string>;
  types?: Record<string, DetailFieldType | undefined>;
  valueMaps?: Record<string, Record<string, string> | undefined>;
};

type DetailDrawerData = {
  data: ApiRecord | ApiRecord[];
  fieldMeta?: DetailDrawerFieldMeta;
  title?: string;
};

type DetailDrawerProps = {
  detail: DetailDrawerData | null;
  onClose: () => void;
};

type FieldRow = {
  field: string;
  value: unknown;
};

const FIELD_LABELS: Record<string, string> = {
  id: 'ID',
  account_id: '账户ID',
  action: '操作',
  actor_id: '触发方ID',
  actor_type: '触发方',
  admin_status: '后台账号状态',
  admin_username: '代理后台账号',
  agent_code: '代理编号',
  agent_id: '代理ID',
  allocated_quantity: '获配数量',
  amount: '金额',
  apr_rate: '年化利率',
  redemption_fee_rate: '提现赎回手续费率',
  maturity_profit_fee_rate: '到期获利手续费率',
  early_redeem_fee_basis: '提前赎回扣费基准',
  early_redeem_fee_rate: '提前赎回扣费率',
  gross_yield_amount: '总收益',
  redemption_fee_amount: '提现赎回手续费',
  maturity_profit_fee_amount: '到期获利手续费',
  early_redeem_fee_amount: '提前赎回扣费',
  fee_amount: '手续费合计',
  redeem_amount: '赎回到账金额',
  user_id: '用户ID',
  admin_id: '管理员ID',
  email: '邮箱',
  phone: '手机号',
  name: '名称',
  status: '状态',
  enabled: '启用',
  kyc_level: 'KYC等级',
  asset_id: '资产ID',
  asset_symbol: '资产',
  asset_type: '资产类型',
  available: '可用',
  balance_after: '变动后余额',
  balance_type: '余额类型',
  base_asset: '基础资产',
  bet_content: '投注内容',
  betContent: '投注内容',
  bet_detail: '投注详情',
  bet_info: '投注信息',
  bet_numbers: '投注号码',
  borrowed_amount: '借款本金',
  buy_order_id: '买单号',
  category: '分类',
  change_type: '变动类型',
  commission_amount: '佣金金额',
  commission_rate: '佣金比例',
  convert_pair_id: '交易对ID',
  country_code: '国家代码',
  country_name: '国家名称',
  remark: '备注',
  created_by: '创建管理员',
  created_at: '创建时间',
  decision: '决策',
  default_locale: '默认语言',
  detail: '详情',
  direction: '方向',
  duration_seconds: '周期秒数',
  entry_price: '入场价',
  equity: '权益',
  event_type: '事件类型',
  expires_at: '到期时间',
  fee: '手续费',
  fee_paid_status: '矿工费状态',
  filled_quantity: '已成交数量',
  frozen: '冻结',
  from_amount: '源金额',
  from_asset_id: '源资产',
  hourly_interest_rate: '小时利率',
  interest_amount: '累计利息',
  ip: 'IP',
  issue_price: '发行价',
  leverage_levels: '杠杆档位',
  level: '层级',
  lifecycle_status: '生命周期',
  locked: '锁定',
  lock_position_id: '锁仓ID',
  locked_amount: '锁定数量',
  margin_amount: '保证金',
  margin_asset: '保证金资产',
  margin_asset_symbol: '保证金资产',
  margin_mode: '保证金模式',
  mark_price: '标记价',
  matures_at: '到期时间',
  max_amount: '最大金额',
  max_stake: '最大押注',
  min_amount: '最小金额',
  target_max_amount: '目标资产最大金额',
  target_min_amount: '目标资产最小金额',
  min_margin: '最小保证金',
  min_order_value: '最小下单额',
  min_stake: '最小押注',
  min_subscribe: '最小申购',
  notional_amount: '名义金额',
  order_count: '订单数',
  order_no: '订单号',
  order_type: '订单类型',
  pair_id: '交易对ID',
  payout_amount: '返还金额',
  payout_rate: '赔率',
  position_count: '仓位数量',
  position_id: '仓位ID',
  price: '价格',
  price_precision: '价格精度',
  pricing_mode: '定价模式',
  product_id: '产品ID',
  product_type: '产品类型',
  project_id: '项目ID',
  published_at: '发布时间',
  qty_precision: '数量精度',
  quantity: '数量',
  quote_amount: '支付金额',
  quote_asset: '支付资产',
  quote_id: '报价ID',
  rate: '汇率',
  reason: '原因',
  ref_id: '来源ID',
  ref_type: '来源类型',
  registration_enabled: '开放注册',
  released_amount: '已释放',
  remaining_amount: '剩余',
  requested_quantity: '申购数量',
  result: '结果',
  risk_level: '风险等级',
  rule_type: '规则类型',
  run_status: '运行状态',
  sell_order_id: '卖单号',
  side: '方向',
  sort_order: '排序',
  source_amount: '来源金额',
  source_id: '来源ID',
  source_type: '来源类型',
  spread_rate: '价差率',
  stake_amount: '押注金额',
  stake_asset_symbol: '押注资产',
  start_price: '起始价',
  strategy_type: '策略类型',
  subscription_id: '申购号',
  supported_locales: '支持语言',
  symbol: '交易对',
  target_id: '对象ID',
  target_price: '目标价',
  target_type: '对象类型',
  term_days: '期限天数',
  ticket_content: '投注内容',
  title: '标题',
  to_amount: '目标金额',
  to_asset_id: '目标资产',
  total_balance: '总余额',
  total_supply: '总量',
  unlock_at: '解禁时间',
  unlock_fee_amount: '矿工费',
  unlock_quantity: '解禁数量',
  unlock_type: '解禁类型',
  updated_at: '更新时间',
  wager_content: '投注内容',
  wager_info: '投注信息'
};

const FIELD_VALUE_LABELS: Record<string, Record<string, string>> = {
  asset_type: {
    coin: '数字货币',
    stablecoin: '稳定币',
    fiat: '法币',
    platform: '平台币'
  },
  balance_type: {
    available: '可用',
    frozen: '冻结',
    locked: '锁定'
  },
  category: {
    general: '通用资讯',
    market: '市场资讯',
    product: '产品资讯',
    system: '系统公告',
    promotion: '活动推广',
    fixed_term: '定期',
    flexible: '活期',
    structured: '结构化',
    staking: '质押'
  },
  decision: {
    allow: '放行',
    deny: '拒绝',
    review: '人工复核'
  },
  default_locale: {
    zh: '中文',
    en: '英文'
  },
  direction: {
    up: '看涨',
    down: '看跌',
    long: '做多',
    short: '做空'
  },
  fee_paid_status: {
    not_required: '无需支付',
    unpaid: '未支付',
    paid: '已支付'
  },
  early_redeem_fee_basis: {
    none: '不扣费',
    principal: '按本金比例扣除',
    profit: '按收益比例扣除'
  },
  lifecycle_status: {
    preheat: '预热中',
    subscription: '发行申购',
    distribution: '派发中',
    listed: '已上市'
  },
  margin_mode: {
    isolated: '逐仓',
    cross: '全仓'
  },
  market_type: {
    external: '外部行情',
    internal: '内部撮合',
    strategy: '策略行情'
  },
  order_type: {
    limit: '限价',
    market: '市价'
  },
  pricing_mode: {
    fixed: '固定汇率',
    market: '市场汇率'
  },
  product_type: {
    convert: '闪兑'
  },
  result: {
    pending: '待处理',
    win: '盈利',
    loss: '亏损'
  },
  risk_level: {
    low: '低风险',
    medium: '中风险',
    high: '高风险',
    critical: '严重风险'
  },
  run_status: {
    pending: '待处理',
    running: '运行中',
    completed: '已完成',
    failed: '失败',
    needs_reload: '待重载'
  },
  side: {
    buy: '买入',
    sell: '卖出'
  },
  source_type: {
    convert: '闪兑'
  },
  status: {
    active: '启用',
    approved: '已通过',
    archived: '已归档',
    cancelled: '已取消',
    completed: '已完成',
    disabled: '禁用',
    draft: '草稿',
    enabled: '启用',
    failed: '失败',
    inactive: '禁用',
    liquidated: '已强平',
    locked: '锁定',
    opened: '持仓中',
    partially_filled: '部分成交',
    pending: '待处理',
    published: '已发布',
    redeemed: '已赎回',
    rejected: '已拒绝',
    settled: '已结算',
    skipped: '已跳过',
    subscribed: '已申购',
    success: '成功',
    suspended: '暂停',
    true: '启用',
    false: '禁用'
  },
  supported_locales: {
    zh: '中文',
    en: '英文'
  },
  unlock_type: {
    immediate_on_listing: '上市立即解禁',
    fixed_time: '固定时间解禁',
    relative_period: '相对周期解禁'
  }
};

function fieldLabel(key: string, meta?: DetailDrawerFieldMeta) {
  return meta?.labels?.[key] ?? FIELD_LABELS[key] ?? key;
}

function mappedValue(value: unknown, key: string, meta?: DetailDrawerFieldMeta): string | null {
  if (value === null || value === undefined || value === '') {
    return null;
  }
  const stringValue = typeof value === 'boolean' ? String(value) : String(value);
  return meta?.valueMaps?.[key]?.[stringValue] ?? FIELD_VALUE_LABELS[key]?.[stringValue.toLowerCase()] ?? FIELD_VALUE_LABELS.status?.[stringValue.toLowerCase()] ?? null;
}

function businessOrderFieldValue(value: unknown, key: string): string | null {
  const prefixes: Record<string, string> = {
    buy_order_id: 'SP',
    sell_order_id: 'SP',
    subscription_id: 'NC'
  };
  const prefix = prefixes[key];
  if (!prefix || value === null || value === undefined || value === '') {
    return null;
  }
  return formatBusinessOrderNo(prefix, { id: value });
}

function typedDisplayValue(value: unknown, key: string, meta?: DetailDrawerFieldMeta): string | null {
  const orderNoValue = businessOrderFieldValue(value, key);
  if (orderNoValue) {
    return orderNoValue;
  }

  const type = meta?.types?.[key];
  if (type === 'timestamp' && typeof value === 'number') {
    return formatAdminTimestamp(value);
  }
  if (type === 'amount') {
    const formatted = typeof value === 'string' || typeof value === 'number' ? formatAdminNumber(value) : null;
    return formatted ? `${formatted}${meta?.assets?.[key] ? ` ${meta.assets[key]}` : ''}` : null;
  }
  if (type === 'status') {
    return mappedValue(value, key, meta);
  }
  return null;
}

function displayValue(value: unknown, key = '', meta?: DetailDrawerFieldMeta): string {
  if (value === null || value === undefined || value === '') {
    return '-';
  }

  if (isAdminBetContentField(key, fieldLabel(key, meta))) {
    const formattedBetContent = formatAdminBetContent(value);
    if (formattedBetContent) {
      return formattedBetContent;
    }
  }

  if (Array.isArray(value)) {
    return value.map((item) => displayValue(item, key, meta)).join(' / ');
  }

  if (typeof value === 'object') {
    return Object.entries(value as ApiRecord)
      .map(([itemKey, item]) => `${fieldLabel(itemKey, meta)}: ${displayValue(item, itemKey, meta)}`)
      .join('；');
  }

  const typed = typedDisplayValue(value, key, meta);
  if (typed) {
    return typed;
  }

  const mapped = mappedValue(value, key, meta);
  if (mapped) {
    return mapped;
  }

  return formatAdminDisplayValue(key, value) ?? String(value);
}

function toRows(record: ApiRecord): FieldRow[] {
  return Object.entries(record).map(([field, value]) => ({ field, value }));
}

function fieldColumns(meta?: DetailDrawerFieldMeta): Array<ColumnProps<FieldRow>> {
  return [
    {
      dataIndex: 'field',
      title: '字段',
      width: 220,
      render: (value: string) => <Text strong>{fieldLabel(value, meta)}</Text>
    },
    {
      dataIndex: 'value',
      title: '内容',
      width: 640,
      render: (value: unknown, row: FieldRow) => <span>{displayValue(value, row.field, meta)}</span>
    }
  ];
}

function recordColumns(records: ApiRecord[], meta?: DetailDrawerFieldMeta): Array<ColumnProps<ApiRecord>> {
  const keys = [...new Set(records.flatMap((record) => Object.keys(record)))];
  return keys.map((key) => ({
    dataIndex: key,
    title: fieldLabel(key, meta),
    width: 180,
    render: (value: unknown) => <span>{displayValue(value, key, meta)}</span>
  }));
}

export function DetailDrawer({ detail, onClose }: DetailDrawerProps) {
  const data = detail?.data;
  const meta = detail?.fieldMeta;
  const records = Array.isArray(data) ? data : [];

  return (
    <SideSheet onCancel={onClose} title={detail?.title ?? '详情'} visible={detail !== null} width="80%">
      {Array.isArray(data) ? (
        <Table
          bordered
          columns={recordColumns(records, meta)}
          dataSource={records}
          pagination={false}
          resizable
          rowKey={(record) => String(record?.id ?? displayValue(record))}
          scroll={containedTableScroll}
          style={containedTableStyle}
        />
      ) : (
        <Table
          bordered
          columns={fieldColumns(meta)}
          dataSource={data ? toRows(data) : []}
          pagination={false}
          resizable
          rowKey="field"
          scroll={containedTableScroll}
          style={containedTableStyle}
        />
      )}
    </SideSheet>
  );
}

export type { DetailDrawerData, DetailDrawerFieldMeta, DetailFieldType };
