import { redactSensitiveText } from '../../shared/sensitiveText';

export const REDACTED_AUDIT_VALUE = '敏感内容已遮罩';

type AuditTargetMeta = {
  href?: string;
  label: string;
};

const AUDIT_TARGET_META: Record<string, AuditTargetMeta> = {
  admin_config_change_request: { label: '高风险配置变更申请', href: '/admin/config-center' },
  admin_news_item: { label: '公告', href: '/admin/news' },
  agent: { label: '代理', href: '/admin/agents' },
  agent_admin_user: { label: '代理管理员', href: '/admin/agents' },
  agent_commission: { label: '代理佣金', href: '/admin/agent-commissions' },
  agent_commission_rule: { label: '代理佣金规则', href: '/admin/agent-commission-rules' },
  asset: { label: '资产', href: '/admin/assets' },
  convert_order: { label: '闪兑订单', href: '/admin/convert/orders' },
  convert_pair: { label: '闪兑交易对', href: '/admin/convert/pairs' },
  country_config: { label: '国家配置', href: '/admin/system/countries' },
  deposit_address_pool: { label: '充值地址池', href: '/admin/wallet/deposit-address-pool' },
  deposit_network_config: { label: '充值网络', href: '/admin/wallet/deposit-network-configs' },
  earn_category: { label: '理财分类', href: '/admin/earn/categories' },
  earn_product: { label: '理财产品', href: '/admin/earn/products' },
  earn_subscription: { label: '理财申购', href: '/admin/earn/subscriptions' },
  event_outbox: { label: '事件队列', href: '/admin/risk/events' },
  kyc_config: { label: 'KYC 规则', href: '/admin/users/kyc/settings' },
  loan_order: { label: '贷款订单', href: '/admin/loan/orders' },
  loan_product: { label: '贷款产品', href: '/admin/loan/products' },
  margin_position: { label: '杠杆仓位', href: '/admin/margin/positions' },
  margin_product: { label: '杠杆产品', href: '/admin/margin/products' },
  market_feed_config: { label: '行情订阅配置', href: '/admin/market/feed-config' },
  market_source_credential: { label: '行情源凭据', href: '/admin/market/feed-config' },
  market_strategy: { label: '行情策略', href: '/admin/market/strategies' },
  new_coin_convert_rule: { label: '新币兑换规则', href: '/admin/new-coins/projects' },
  new_coin_distribution: { label: '新币派发', href: '/admin/new-coins/distributions' },
  new_coin_project: { label: '新币项目', href: '/admin/new-coins/projects' },
  platform_brand_config: { label: '平台品牌', href: '/admin/system/brand' },
  prediction_asset_config: { label: '竞猜资产配置', href: '/admin/prediction/settings?tab=assets' },
  prediction_settings: { label: '竞猜全局设置', href: '/admin/prediction/settings' },
  quick_recharge_config: { label: '快速充值配置', href: '/admin/wallet/quick-recharge' },
  quick_recharge_order: { label: '快速充值订单', href: '/admin/wallet/quick-recharge-orders' },
  risk_rule: { label: '风控规则', href: '/admin/risk' },
  seconds_contract_order: { label: '秒合约订单', href: '/admin/seconds-contract/orders' },
  seconds_contract_product: { label: '秒合约产品', href: '/admin/seconds-contract/products' },
  security_policy: { label: '安全策略', href: '/admin/system/security-policy' },
  smtp_config: { label: 'SMTP 配置', href: '/admin/system/smtp' },
  smtp_delivery_settings: { label: 'SMTP 发信策略', href: '/admin/system/smtp' },
  spot_order: { label: '现货订单', href: '/admin/spot/orders' },
  spot_trade: { label: '现货成交', href: '/admin/spot/trades' },
  trading_pair: { label: '交易对', href: '/admin/market/pairs' },
  upload_storage_config: { label: '上传存储配置', href: '/admin/system/uploads' },
  user: { label: '用户', href: '/admin/users' },
  user_kyc_submission: { label: 'KYC 申请', href: '/admin/users/kyc/reviews' },
  user_referral: { label: '用户代理关系', href: '/admin/users' },
  user_two_factor: { label: '用户两步验证', href: '/admin/users' },
  wallet_account: { label: '钱包账户', href: '/admin/wallet/accounts' }
};

const AUDIT_ACTION_LABELS: Record<string, string> = {
  'agent_admin_user.password.reset': '重置代理管理员密码',
  'config_change.applied': '应用高风险配置变更',
  'config_change.approved': '通过高风险配置变更',
  'config_change.rejected': '驳回高风险配置变更',
  'config_change.requested': '提交高风险配置变更',
  'event_outbox.requeue': '重排失败事件',
  'kyc.config.update': '更新 KYC 规则',
  'kyc.submission.approve': '通过 KYC 申请',
  'kyc.submission.reject': '驳回 KYC 申请',
  'market_strategy.kline_recovery.completed': '完成行情 K 线补偿',
  'market_strategy.kline_recovery.execute': '执行行情 K 线补偿',
  'market_strategy.kline_recovery.failed': '行情 K 线补偿失败',
  'market_strategy.kline_recovery.requested': '申请行情 K 线补偿',
  'market_strategy.version.restore': '恢复行情策略版本',
  'market_strategy.version.restored': '完成行情策略版本恢复',
  'new_coin_project.lifecycle.update': '变更新币项目生命周期',
  'new_coin_project.post_listing_purchase.update': '更新新币上市认购规则',
  'new_coin_project.unlock_fee_rule.update': '更新新币解禁费用规则',
  'new_coin_project.unlock_rule.update': '更新新币解禁规则',
  'seconds_contract_order.settle': '人工结算秒合约订单',
  'spot_order.cancel': '取消现货订单',
  'user_2fa.reset': '重置用户两步验证',
  'user_referral.assign_agent': '分配用户代理',
  'wallet.recharge': '人工充值用户钱包'
};

const AUDIT_FIELD_LABELS: Record<string, string> = {
  action: '操作动作',
  active_version: '当前生效版本',
  admin_id: '管理员 ID',
  admin_status: '管理员状态',
  admin_user_id: '管理员账号 ID',
  admin_username: '管理员用户名',
  allowed_asset_ids: '允许资产',
  amount: '金额',
  api_key: 'API 密钥',
  api_key_mask: 'API 密钥掩码',
  applied_version: '已应用版本',
  asset_id: '资产 ID',
  asset_symbol: '资产符号',
  auth_type: '认证方式',
  available_balance: '可用余额',
  base_asset: '基础资产',
  base_asset_id: '基础资产 ID',
  base_revision: '基础版本',
  category: '分类',
  child_agent_count: '下级代理数',
  code: '代码',
  config_domain: '配置域',
  config_json: '配置内容',
  config_version: '配置版本',
  created_at: '创建时间',
  created_by: '创建管理员',
  current_price: '当前价格',
  default_fee_rate: '默认手续费率',
  default_invalid_refund_policy: '默认无效退款策略',
  default_settlement_mode: '默认结算方式',
  depth: '层级深度',
  direct_inviter_id: '直接邀请人 ID',
  direct_inviter_type: '直接邀请人类型',
  direct_user_count: '直属用户数',
  email: '邮箱',
  enabled: '启用状态',
  end_time: '结束时间',
  fee_rate: '手续费率',
  host: '服务器地址',
  id: 'ID',
  intervals: '订阅周期',
  invalid_refund_policy: '无效退款策略',
  last_generated_at: '最后生成时间',
  last_kline_open_time: '最后 K 线时间',
  last_reload_error: '最近重载错误',
  last_reload_status: '最近重载状态',
  last_reloaded_at: '最近重载时间',
  level: '等级',
  login_2fa_mode: '登录两步验证模式',
  logo_url: 'Logo 地址',
  margin_transfer_enabled: '允许转入杠杆账户',
  market_type: '行情类型',
  max_amount: '最大金额',
  max_payout_amount: '最大赔付金额',
  min_amount: '最小金额',
  min_order_value: '最小下单额',
  name: '名称',
  order_id: '订单 ID',
  pair_id: '交易对 ID',
  parent_agent_code: '上级代理代码',
  parent_agent_id: '上级代理 ID',
  password: '密码',
  path: '层级路径',
  payout_amount: '赔付金额',
  port: '端口',
  price_precision: '价格精度',
  proposed_json: '拟变更内容',
  provider: '服务商',
  providers: '服务商列表',
  qty_precision: '数量精度',
  quote_asset: '计价资产',
  quote_asset_id: '计价资产 ID',
  quote_ttl_seconds: '报价有效秒数',
  reason: '原因',
  recovery_status: '补偿状态',
  registration_invite_required: '注册必须邀请码',
  revision: '修订版本',
  risk_level: '风险等级',
  root_agent_code: '根代理代码',
  root_agent_id: '根代理 ID',
  run_status: '运行状态',
  runtime: '运行状态详情',
  secret: '密钥',
  settlement_mode: '结算方式',
  start_price: '起始价格',
  start_time: '开始时间',
  status: '状态',
  strategy_type: '策略类型',
  symbol: '符号',
  symbols: '订阅交易对',
  sync_enabled: '同步开关',
  sync_interval_seconds: '同步间隔秒数',
  sync_tags: '同步标签',
  target_id: '对象 ID',
  target_max_amount: '目标最大金额',
  target_min_amount: '目标最小金额',
  target_price: '目标价格',
  target_type: '对象类型',
  team_user_count: '团队用户数',
  third_party_bindings: '第三方绑定',
  token: '令牌',
  totp_enabled: 'TOTP 启用状态',
  updated_at: '更新时间',
  updated_by: '更新管理员',
  user_id: '用户 ID',
  username: '用户名',
  username_login_enabled: '用户名登录开关',
  version: '版本',
  volatility: '波动率',
  volume_max: '最大成交量',
  volume_min: '最小成交量'
};

const AUDIT_FIELD_WORDS: Record<string, string> = {
  account: '账户',
  active: '生效',
  address: '地址',
  admin: '管理员',
  agent: '代理',
  amount: '金额',
  applied: '已应用',
  asset: '资产',
  at: '时间',
  balance: '余额',
  base: '基础',
  category: '分类',
  config: '配置',
  count: '数量',
  created: '创建',
  default: '默认',
  enabled: '启用状态',
  end: '结束',
  error: '错误',
  fee: '手续费',
  id: 'ID',
  interval: '间隔',
  last: '最近',
  max: '最大',
  min: '最小',
  mode: '模式',
  name: '名称',
  order: '订单',
  pair: '交易对',
  price: '价格',
  product: '产品',
  quote: '计价',
  rate: '费率',
  revision: '修订版本',
  status: '状态',
  strategy: '策略',
  symbol: '符号',
  target: '目标',
  time: '时间',
  type: '类型',
  updated: '更新',
  user: '用户',
  value: '值',
  version: '版本'
};

const AUDIT_VALUE_LABELS: Record<string, string> = {
  active: '启用',
  approved: '已通过',
  auto: '自动',
  cancelled: '已取消',
  closed: '已关闭',
  completed: '已完成',
  development: '开发环境',
  disabled: '停用',
  draft: '草稿',
  error: '异常',
  failed: '失败',
  fixed: '固定',
  healthy: '健康',
  idle: '空闲',
  open: '进行中',
  pending: '待处理',
  production: '生产环境',
  refunded: '已退款',
  rejected: '已驳回',
  running: '运行中',
  settled: '已结算',
  staging: '预发布环境',
  stopped: '已停止',
  success: '成功',
  suspended: '已暂停',
  test: '测试环境'
};

const SENSITIVE_KEY_PARTS = new Set([
  'credential',
  'credentials',
  'ciphertext',
  'key',
  'keys',
  'passphrase',
  'password',
  'passwords',
  'secret',
  'secrets',
  'token',
  'tokens'
]);

const MISSING_VALUE = Symbol('missing-audit-value');
type MissingValue = typeof MISSING_VALUE;
type AuditPathSegment = number | string;

export type AuditFieldChange = {
  after: string;
  before: string;
  label: string;
  path: string;
  sensitive: boolean;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizedKeyParts(key: string): string[] {
  return key
    .replace(/([a-z\d])([A-Z])/g, '$1_$2')
    .toLowerCase()
    .split(/[^a-z\d]+/u)
    .filter(Boolean);
}

export function isSensitiveAuditKey(key: string): boolean {
  return normalizedKeyParts(key).some((part) => SENSITIVE_KEY_PARTS.has(part));
}

/**
 * 对原因、备注等非结构化审计文本做保守脱敏，避免历史记录中的键值凭据绕过 JSON 字段遮罩。
 * 普通业务文字保持原样；命名凭据赋值和 Bearer 令牌仅保留字段名或认证方案。
 */
export function redactAuditFreeText(value: string): string {
  return redactSensitiveText(value);
}

export function redactAuditSnapshot(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(redactAuditSnapshot);
  }
  if (typeof value === 'string') {
    return redactAuditFreeText(value);
  }
  if (!isRecord(value)) {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [
      key,
      isSensitiveAuditKey(key) ? REDACTED_AUDIT_VALUE : redactAuditSnapshot(child)
    ])
  );
}

export function auditTargetLabel(targetType: string): string {
  return AUDIT_TARGET_META[targetType]?.label
    ?? `其他后台对象（${redactAuditFreeText(targetType || '未标识')}）`;
}

export function auditTargetHref(targetType: string): string | null {
  return AUDIT_TARGET_META[targetType]?.href ?? null;
}

export function auditActionLabel(action: string, targetType: string): string {
  const exactLabel = AUDIT_ACTION_LABELS[action];
  if (exactLabel) {
    return exactLabel;
  }

  const target = auditTargetLabel(targetType);
  if (action.endsWith('.status.update') || action.endsWith('.update_status')) {
    return `变更${target}状态`;
  }
  if (action.endsWith('.config.update') || action.endsWith('.update')) {
    return `更新${target}`;
  }
  if (action.endsWith('.create') || action.endsWith('.created')) {
    return `创建${target}`;
  }
  if (action.endsWith('.delete')) {
    return `删除${target}`;
  }
  if (action.endsWith('.save') || action.endsWith('.upsert')) {
    return `保存${target}`;
  }
  if (action.endsWith('.test')) {
    return `测试${target}`;
  }
  if (action.endsWith('.reload')) {
    return `重载${target}`;
  }
  if (action.endsWith('.reclaim')) {
    return `回收${target}`;
  }
  if (action.endsWith('.approve') || action.endsWith('.approved')) {
    return `通过${target}`;
  }
  if (action.endsWith('.reject') || action.endsWith('.rejected')) {
    return `驳回${target}`;
  }
  if (action.endsWith('.cancel') || action.endsWith('.cancelled')) {
    return `取消${target}`;
  }
  if (action.endsWith('.settle') || action.endsWith('.settled')) {
    return `结算${target}`;
  }
  return `操作${target}`;
}

function auditFieldSegmentLabel(segment: AuditPathSegment): string {
  if (typeof segment === 'number') {
    return `第 ${segment + 1} 项`;
  }

  const exact = AUDIT_FIELD_LABELS[segment];
  if (exact) {
    return exact;
  }

  const parts = normalizedKeyParts(segment);
  if (parts.length > 0 && parts.every((part) => AUDIT_FIELD_WORDS[part])) {
    return parts.map((part) => AUDIT_FIELD_WORDS[part]).join('');
  }
  return `字段「${redactAuditFreeText(segment)}」`;
}

function auditFieldLabel(path: AuditPathSegment[]): string {
  return path.length > 0 ? path.map(auditFieldSegmentLabel).join(' / ') : '记录内容';
}

function auditFieldPath(path: AuditPathSegment[]): string {
  return path.reduce<string>((result, segment) => {
    if (typeof segment === 'number') {
      return `${result}[${segment}]`;
    }
    return result ? `${result}.${segment}` : segment;
  }, '');
}

function jsonEqual(left: unknown | MissingValue, right: unknown | MissingValue): boolean {
  if (Object.is(left, right)) {
    return true;
  }
  if (left === MISSING_VALUE || right === MISSING_VALUE) {
    return false;
  }
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.length === right.length && left.every((value, index) => jsonEqual(value, right[index]));
  }
  if (isRecord(left) && isRecord(right)) {
    const leftKeys = Object.keys(left).sort();
    const rightKeys = Object.keys(right).sort();
    return leftKeys.length === rightKeys.length
      && leftKeys.every((key, index) => key === rightKeys[index] && jsonEqual(left[key], right[key]));
  }
  return false;
}

function isTimestampPath(path: AuditPathSegment[]): boolean {
  const last = path.at(-1);
  return typeof last === 'string' && (last.endsWith('_at') || last.endsWith('_time'));
}

function formatTimestamp(value: number): string {
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false
  }).format(new Date(value));
}

function formatAuditValue(value: unknown | MissingValue, path: AuditPathSegment[], sensitive: boolean): string {
  if (value === MISSING_VALUE) {
    return '未设置';
  }
  if (sensitive) {
    return REDACTED_AUDIT_VALUE;
  }
  if (value === null || value === undefined) {
    return '未设置';
  }
  if (typeof value === 'boolean') {
    return value ? '是' : '否';
  }
  if (typeof value === 'number') {
    if (Number.isFinite(value) && value > 0 && isTimestampPath(path)) {
      return formatTimestamp(value);
    }
    return Number.isFinite(value) ? String(value) : '无效数值';
  }
  if (typeof value === 'string') {
    if (value.length === 0) {
      return '空字符串';
    }
    const mapped = AUDIT_VALUE_LABELS[value.toLowerCase()];
    if (mapped) {
      return mapped;
    }
    const singleLine = redactAuditFreeText(value).replace(/\s+/gu, ' ').trim();
    return singleLine.length > 160 ? `${singleLine.slice(0, 160)}…` : singleLine;
  }
  if (Array.isArray(value)) {
    return `列表（${value.length} 项）`;
  }
  if (isRecord(value)) {
    return `对象（${Object.keys(value).length} 个字段）`;
  }
  return '不支持的值';
}

function pushAuditChanges(
  changes: AuditFieldChange[],
  before: unknown | MissingValue,
  after: unknown | MissingValue,
  path: AuditPathSegment[]
): void {
  if (jsonEqual(before, after)) {
    return;
  }

  const last = path.at(-1);
  const sensitive = typeof last === 'string' && isSensitiveAuditKey(last);
  if (sensitive) {
    changes.push({
      after: formatAuditValue(after, path, true),
      before: formatAuditValue(before, path, true),
      label: auditFieldLabel(path),
      path: auditFieldPath(path),
      sensitive: true
    });
    return;
  }

  const beforeRecord = isRecord(before) ? before : null;
  const afterRecord = isRecord(after) ? after : null;
  if ((beforeRecord || before === MISSING_VALUE || before === null) && (afterRecord || after === MISSING_VALUE || after === null)
    && (beforeRecord || afterRecord)) {
    const keys = [...new Set([...Object.keys(beforeRecord ?? {}), ...Object.keys(afterRecord ?? {})])].sort();
    if (keys.length > 0) {
      keys.forEach((key) => {
        pushAuditChanges(
          changes,
          beforeRecord && Object.hasOwn(beforeRecord, key) ? beforeRecord[key] : MISSING_VALUE,
          afterRecord && Object.hasOwn(afterRecord, key) ? afterRecord[key] : MISSING_VALUE,
          [...path, key]
        );
      });
      return;
    }
  }

  const beforeArray = Array.isArray(before) ? before : null;
  const afterArray = Array.isArray(after) ? after : null;
  if ((beforeArray || before === MISSING_VALUE || before === null) && (afterArray || after === MISSING_VALUE || after === null)
    && (beforeArray || afterArray)) {
    const length = Math.max(beforeArray?.length ?? 0, afterArray?.length ?? 0);
    if (length > 0) {
      for (let index = 0; index < length; index += 1) {
        pushAuditChanges(
          changes,
          beforeArray && index < beforeArray.length ? beforeArray[index] : MISSING_VALUE,
          afterArray && index < afterArray.length ? afterArray[index] : MISSING_VALUE,
          [...path, index]
        );
      }
      return;
    }
  }

  changes.push({
    after: formatAuditValue(after, path, false),
    before: formatAuditValue(before, path, false),
    label: auditFieldLabel(path),
    path: auditFieldPath(path),
    sensitive: false
  });
}

export function buildAuditFieldChanges(before: unknown, after: unknown): AuditFieldChange[] {
  const changes: AuditFieldChange[] = [];
  pushAuditChanges(changes, before ?? MISSING_VALUE, after ?? MISSING_VALUE, []);
  return changes;
}
