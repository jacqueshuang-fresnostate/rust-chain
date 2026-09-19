import { apiRequest, ContractError } from '../../api/client';
import type { ApiRecord } from '../../api/types';

export const FINANCIAL_RETRIES_PATH = '/admin/api/v1/governance/financial-retries';
export const retryKinds = { earn: '理财到期赎回', loan: '贷款逾期回收', commission: '代理佣金结算', seconds: '秒合约人工复核' };
export const retryOutcomes = {
  ready: '待重试', running: '执行中', waiting_balance: '等待余额', waiting_source: '等待来源终态', failed: '执行失败', manual_review: '待人工复核'
};
export const leaseStatuses = { active: '租约有效', expired: '租约已过期', none: '无租约' };
export const sourceTypes: Record<string, string> = {
  earn_subscription: '理财申购', loan_order: '贷款订单', convert_order: '闪兑订单',
  prediction_order: '竞猜订单', spot_order: '现货订单', spot_trade: '现货成交',
  seconds_contract_order: '秒合约订单', margin_order: '杠杆订单', margin_position: '杠杆仓位'
};

export type FinancialRetrySchedule = {
  version: string;
  task_kind: keyof typeof retryKinds;
  item_id: number;
  outcome: keyof typeof retryOutcomes;
  attempt_count: number | null;
  last_attempt_at: number | null;
  next_attempt_at: number | null;
  lease_status: keyof typeof leaseStatuses;
};
export type FinancialRetry = FinancialRetrySchedule & {
  source_type: string | null;
  source_order_id: string | null;
  user_id: number | null;
  amount: string | null;
  asset: string | null;
  incident: FinancialRetryIncident;
  failure_code: string | null;
  failure_at: number | null;
  review_window_start: number | null;
  review_window_end: number | null;
};
export type FinancialRetryIncident = {
  version: number;
  owner_admin_id: number | null;
  owner_name: string | null;
  due_at: number | null;
  updated_by: number | null;
  updated_at: number | null;
};
export type FinancialRetries = {
  retries: FinancialRetry[];
  total: number;
  counts: { outcome: keyof typeof retryOutcomes; count: number }[];
  limit: number;
  offset: number;
  checked_at: number;
};

function invalid(): never {
  throw new ContractError('资金异常响应不完整，请刷新后重试', { path: FINANCIAL_RETRIES_PATH });
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return invalid();
  return value as Record<string, unknown>;
}
function integer(value: unknown, min = 0): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min;
}
function timestamp(value: unknown): value is number {
  return integer(value, 1) && value <= 8_640_000_000_000_000;
}
function member(value: unknown, labels: Record<string, string>): boolean {
  return typeof value === 'string' && Object.hasOwn(labels, value);
}
function nullableText(value: unknown): boolean {
  return value === null || (typeof value === 'string' && value.trim().length > 0);
}

export function parseFinancialRetrySchedule(value: unknown): FinancialRetrySchedule {
  const row = object(value);
  if (typeof row.version !== 'string' || !/^[a-f0-9]{64}$/.test(row.version)
    || !member(row.task_kind, retryKinds) || !member(row.outcome, retryOutcomes)
    || !member(row.lease_status, leaseStatuses) || !integer(row.item_id, 1)
    || !(row.task_kind === 'seconds'
      ? row.outcome === 'manual_review' && row.attempt_count === null && row.next_attempt_at === null && row.last_attempt_at === null && row.lease_status === 'none'
      : integer(row.attempt_count) && timestamp(row.next_attempt_at) && row.outcome !== 'manual_review')
    || !(row.last_attempt_at === null || timestamp(row.last_attempt_at))) return invalid();
  return row as FinancialRetrySchedule;
}

export function parseFinancialRetryIncident(value: unknown): FinancialRetryIncident {
  const row = object(value);
  if (!integer(row.version) || !(row.owner_admin_id === null || integer(row.owner_admin_id, 1))
    || !nullableText(row.owner_name) || ((row.owner_admin_id === null) !== (row.owner_name === null))
    || !(row.due_at === null || (typeof row.due_at === 'number' && Number.isSafeInteger(row.due_at)
      && row.due_at >= -30610224000000 && row.due_at <= 253402300799999))
    || !(row.version === 0
      ? row.updated_by === null && row.updated_at === null && row.owner_admin_id === null && row.due_at === null
      : integer(row.updated_by, 1) && timestamp(row.updated_at))) return invalid();
  return row as FinancialRetryIncident;
}

/** 原始金额和来源 ID 不经 Number 转换；遗漏字段或计数不一致时失败关闭。 */
export function parseFinancialRetries(value: unknown): FinancialRetries {
  const data = object(value);
  if (!Array.isArray(data.retries) || !Array.isArray(data.counts) || !integer(data.total)
    || !integer(data.limit, 1) || data.limit > 100 || !integer(data.offset) || data.offset > 100_000
    || !timestamp(data.checked_at) || data.retries.length > data.limit
    || data.retries.length !== Math.min(data.limit, Math.max(0, data.total - data.offset))) return invalid();
  const keys = new Set<string>();
  for (const value of data.retries) {
    const row = object(value);
    parseFinancialRetrySchedule(row);
    parseFinancialRetryIncident(row.incident);
    if (!nullableText(row.source_type) || !nullableText(row.source_order_id)
      || !nullableText(row.asset) || !(row.user_id === null || integer(row.user_id, 1))
      || !(row.amount === null || (typeof row.amount === 'string' && /^\d+(?:\.\d+)?$/.test(row.amount)))) return invalid();
    if (!nullableText(row.failure_code)
      || ![row.failure_at, row.review_window_start, row.review_window_end].every((v) => v === null || timestamp(v))) return invalid();
    if (row.task_kind === 'seconds' && (row.failure_code === null || !timestamp(row.failure_at)
      || !timestamp(row.review_window_start) || !timestamp(row.review_window_end)
      || row.review_window_end <= row.review_window_start || row.source_type !== 'seconds_contract_order'
      || row.source_order_id !== String(row.item_id) || row.user_id === null || row.amount === null || row.asset === null)) return invalid();
    const key = `${row.task_kind}:${row.item_id}`;
    if (keys.has(key)) return invalid();
    keys.add(key);
  }
  const outcomes = new Set<string>();
  let total = 0;
  for (const value of data.counts) {
    const row = object(value);
    if (!member(row.outcome, retryOutcomes) || !integer(row.count, 1)
      || outcomes.has(String(row.outcome))) return invalid();
    outcomes.add(String(row.outcome));
    total += row.count;
  }
  if (total !== data.total) return invalid();
  return data as FinancialRetries;
}

export function financialRetryRequeuePath(row: FinancialRetrySchedule): string {
  return `${FINANCIAL_RETRIES_PATH}/${row.task_kind}/${row.item_id}/requeue`;
}

export async function loadFinancialRetries(filters: Record<string, string>, page: number, signal?: AbortSignal) {
  const params = new URLSearchParams({ ...filters, limit: '20', offset: String((page - 1) * 20) });
  return parseFinancialRetries(await apiRequest<unknown>(`${FINANCIAL_RETRIES_PATH}?${params}`, { signal }));
}

export async function requeueFinancialRetry(row: FinancialRetrySchedule, reason: string) {
  if (row.task_kind === 'seconds' || row.lease_status === 'active') return invalid();
  const response = parseFinancialRetrySchedule(await apiRequest<unknown>(financialRetryRequeuePath(row), {
    method: 'POST', body: JSON.stringify({ reason, expected_version: row.version })
  }));
  if (response.task_kind !== row.task_kind || response.item_id !== row.item_id
    || response.version === row.version || response.outcome !== 'ready' || response.lease_status !== 'none'
    || response.attempt_count !== row.attempt_count || response.last_attempt_at !== row.last_attempt_at) return invalid();
  return response;
}

export async function updateFinancialRetryIncident(
  row: FinancialRetry, ownerAdminId: number | null, dueAt: number | null, reason: string
) {
  const response = parseFinancialRetryIncident(await apiRequest<unknown>(
    `${FINANCIAL_RETRIES_PATH}/${row.task_kind}/${row.item_id}/incident`, {
      method: 'PATCH',
      body: JSON.stringify({ expected_version: row.incident.version, owner_admin_id: ownerAdminId, due_at: dueAt, reason })
    }));
  if (response.version !== row.incident.version + 1 || response.owner_admin_id !== ownerAdminId || response.due_at !== dueAt) return invalid();
  return response;
}

export async function loadFinancialRetrySecondsOrder(id: string, signal?: AbortSignal) {
  if (!/^[1-9]\d*$/.test(id) || !Number.isSafeInteger(Number(id))) return invalid();
  const row = object(await apiRequest<unknown>(`/admin/api/v1/seconds-contracts/orders/${id}`, { signal }));
  if (row.id !== Number(id) || !integer(row.user_id, 1) || typeof row.status !== 'string' || !row.status.trim()
    || typeof row.stake_amount !== 'string' || !/^\d+(?:\.\d+)?$/.test(row.stake_amount)) return invalid();
  return row as ApiRecord;
}
