import { apiRequest, ContractError } from '../../api/client';
import { authStore } from '../../auth/authStore';
import type { ApiRecord } from '../../api/types';
import { canonicalDecimalText } from '../../shared/decimal';

export type NewCoinProject = ApiRecord & {
  id: number; asset_id: number; symbol: string; lifecycle_status: string; status: string;
  quote_asset_id: number | null; total_supply: string; issue_price: string;
  reserved_supply: string; allocated_supply: string; remaining_supply: string;
  unlock_type: string; listed_at: number | null; actual_listed_at: number | null; fixed_unlock_at: number | null;
  relative_unlock_seconds: number | null; unlock_fee_enabled: boolean;
  unlock_fee_rate: string | null; unlock_fee_basis: string | null; unlock_fee_asset: number | null;
  post_listing_purchase_enabled: boolean; post_listing_pair_id: number | null;
};
export type ProjectCenter = {
  configuration_version: string; project: NewCoinProject; subscription_count: number; pending_manual_count: number;
  issuance_editable: boolean; next_lifecycle_status: string | null; lifecycle_block_reason: string | null;
};
export const stages = [
  { value: 'preheat', label: '预热中', action: '开始申购' },
  { value: 'subscription', label: '申购中', action: '结束申购' },
  { value: 'distribution', label: '派发中', action: '确认上市' },
  { value: 'listed', label: '已上市', action: '' }
];
export const projectPath = (id: string | number) => `/admin/new-coins/projects/${encodeURIComponent(id)}`;
export const projectQueryKey = (id: string | number) => {
  const session = authStore.getSession('admin');
  return ['admin-new-coin-project', session ? `${session.subject}:${session.generation}` : 'anonymous:none', String(id)];
};

export async function loadProjectCenter(id: string, signal?: AbortSignal): Promise<ProjectCenter> {
  const path = `/admin/api/v1/new-coins/${id}`;
  const response = await apiRequest<unknown>(path, { signal });
  const fail = () => { throw new ContractError('新币项目配置响应不完整，请刷新或联系管理员', { path }); };
  if (!response || typeof response !== 'object') return fail();
  const data = response as Record<string, unknown>;
  if (typeof data.configuration_version !== 'string') return fail();
  const p = data.project as Record<string, unknown> | undefined;
  if (!p || typeof p !== 'object' || String(p.id) !== id) return fail();
  for (const key of ['id', 'asset_id']) if (!Number.isSafeInteger(p[key]) || Number(p[key]) <= 0) return fail();
  for (const key of ['symbol', 'lifecycle_status', 'status', 'unlock_type']) if (typeof p[key] !== 'string') return fail();
  if (!stages.some(s => s.value === p.lifecycle_status)) return fail();
  for (const key of ['total_supply', 'issue_price', 'reserved_supply', 'allocated_supply', 'remaining_supply']) {
    if (typeof p[key] !== 'string' || canonicalDecimalText(p[key]) === null) return fail();
  }
  for (const key of ['quote_asset_id', 'listed_at', 'actual_listed_at', 'fixed_unlock_at', 'relative_unlock_seconds', 'unlock_fee_asset', 'post_listing_pair_id']) {
    if (p[key] !== null && (!Number.isSafeInteger(p[key]) || Number(p[key]) <= 0)) return fail();
  }
  if (p.unlock_fee_rate !== null && (typeof p.unlock_fee_rate !== 'string' || canonicalDecimalText(p.unlock_fee_rate) === null)) return fail();
  if (p.unlock_fee_basis !== null && typeof p.unlock_fee_basis !== 'string') return fail();
  if (typeof p.unlock_fee_enabled !== 'boolean' || typeof p.post_listing_purchase_enabled !== 'boolean' || typeof data.issuance_editable !== 'boolean') return fail();
  for (const key of ['subscription_count', 'pending_manual_count']) if (!Number.isSafeInteger(data[key]) || Number(data[key]) < 0) return fail();
  if (data.next_lifecycle_status !== null && !stages.some(s => s.value === data.next_lifecycle_status)) return fail();
  if (data.lifecycle_block_reason !== null && typeof data.lifecycle_block_reason !== 'string') return fail();
  return data as ProjectCenter;
}

/** 原始毫秒精度回填本地时间输入；不经过 UTC 文本截断。 */
export function projectLocalTime(value: number | null): string {
  if (value === null) return '';
  const d = new Date(value);
  const pad = (n: number, width = 2) => String(n).padStart(width, '0');
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(),3)}`;
}
