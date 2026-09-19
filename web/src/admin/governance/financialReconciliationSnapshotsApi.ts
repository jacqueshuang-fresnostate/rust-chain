import { apiRequest, ContractError } from '../../api/client';
import { authStore, type AuthSession } from '../../auth/authStore';
import { FinancialCommandIntentStore, financialCommandScopeFromSession, runRecoverableFinancialCommand } from '../../shared/idempotency';
import { parseReconciliationReport, RECONCILIATION_PATH, type ReconciliationReport } from './financialReconciliationApi';

export const SNAPSHOTS_PATH = `${RECONCILIATION_PATH}/snapshots`;
export type SnapshotSummary = {
  id: number; asset_id: number; asset_symbol: string; precision_scale: number; schema_version: 1;
  captured_at: number; admin_id: number; reason: string;
};
export type SnapshotDetail = SnapshotSummary & {
  idempotency_key: string; report_hash: string; report: ReconciliationReport;
};
export type SnapshotHistory = { snapshots: SnapshotSummary[]; total: number; limit: number; offset: number };
export type FollowupRecord = {
  id: number; snapshot_id: number; version: number; owner_admin_id: number | null;
  due_at: number | null; notes: string; admin_id: number; reason: string;
  recorded_at: number; idempotency_key: string;
};
export type FollowupHistory = {
  snapshot_id: number; latest_version: number; records: FollowupRecord[]; total: number; limit: number; offset: number;
};
export type FollowupDraft = { expected_version: number; owner_admin_id: number | null; due_at: number | null; notes: string };

function invalid(): never {
  throw new ContractError('采集或跟进证据不完整，请保留原请求并重试核对', { path: SNAPSHOTS_PATH });
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return invalid();
  return value as Record<string, unknown>;
}
function integer(value: unknown, min = 0): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min;
}
function text(value: unknown, max: number): value is string {
  return typeof value === 'string' && value.trim().length > 0 && [...value].length <= max;
}
function time(value: unknown): value is number {
  return integer(value, 1) && value <= 253_402_300_799_999;
}
function key(value: unknown): value is string {
  return typeof value === 'string' && /^[!-~]{1,128}$/.test(value);
}
function summary(value: unknown, assetId?: number): SnapshotSummary {
  const row = object(value);
  if (!integer(row.id, 1) || !integer(row.asset_id, 1) || (assetId !== undefined && row.asset_id !== assetId)
    || !text(row.asset_symbol, 64) || !integer(row.precision_scale) || row.precision_scale > 18
    || row.schema_version !== 1 || !time(row.captured_at) || !integer(row.admin_id, 1) || !text(row.reason, 500)) return invalid();
  return row as SnapshotSummary;
}
function page(value: Record<string, unknown>, rows: unknown, requestedPage: number): unknown[] {
  if (!integer(value.total) || value.limit !== 20 || value.offset !== (requestedPage - 1) * 20
    || !Array.isArray(rows) || rows.length !== Math.min(20, Math.max(0, value.total - Number(value.offset)))) return invalid();
  return rows;
}

export function parseSnapshot(value: unknown, id?: number, assetId?: number): SnapshotDetail {
  const row = object(value);
  const data = summary(row, assetId);
  if ((id !== undefined && data.id !== id) || !key(row.idempotency_key)
    || typeof row.report_hash !== 'string' || !/^[a-f0-9]{64}$/.test(row.report_hash)) return invalid();
  const report = parseReconciliationReport(row.report, data.asset_id);
  if (report.asset.symbol !== data.asset_symbol || report.asset.precision_scale !== data.precision_scale) return invalid();
  return row as SnapshotDetail;
}

export function parseSnapshotHistory(value: unknown, requestedPage: number, assetId: number): SnapshotHistory {
  const data = object(value);
  const rows = page(data, data.snapshots, requestedPage).map((row) => summary(row, assetId));
  if (rows.some((row, index) => index > 0 && row.id >= rows[index - 1].id)) return invalid();
  return data as SnapshotHistory;
}

export function parseFollowup(value: unknown, snapshotId: number): FollowupRecord {
  const row = object(value);
  if (!integer(row.id, 1) || row.snapshot_id !== snapshotId || !integer(row.version, 1)
    || !(row.owner_admin_id === null || integer(row.owner_admin_id, 1))
    || !(row.due_at === null || (typeof row.due_at === 'number' && Number.isSafeInteger(row.due_at)
      && row.due_at >= -30_610_224_000_000 && row.due_at <= 253_402_300_799_999))
    || !text(row.notes, 2000) || !integer(row.admin_id, 1) || !text(row.reason, 500)
    || !time(row.recorded_at) || !key(row.idempotency_key)) return invalid();
  return row as FollowupRecord;
}

export function parseFollowupHistory(value: unknown, snapshotId: number, requestedPage: number): FollowupHistory {
  const data = object(value);
  if (data.snapshot_id !== snapshotId || !integer(data.latest_version) || data.latest_version !== data.total) return invalid();
  const rows = page(data, data.records, requestedPage).map((row) => parseFollowup(row, snapshotId));
  if (rows.some((row, index) => row.version !== Number(data.latest_version) - Number(data.offset) - index)
    || new Set(rows.map((row) => row.id)).size !== rows.length) return invalid();
  return data as FollowupHistory;
}

export async function loadSnapshotHistory(assetId: number, pageNumber: number, signal?: AbortSignal) {
  return parseSnapshotHistory(await apiRequest<unknown>(
    `${SNAPSHOTS_PATH}?asset_id=${assetId}&limit=20&offset=${(pageNumber - 1) * 20}`, { signal }
  ), pageNumber, assetId);
}
export async function loadSnapshot(id: number, assetId: number, signal?: AbortSignal) {
  return parseSnapshot(await apiRequest<unknown>(`${SNAPSHOTS_PATH}/${id}`, { signal }), id, assetId);
}
export async function loadFollowups(id: number, pageNumber: number, signal?: AbortSignal) {
  return parseFollowupHistory(await apiRequest<unknown>(
    `${SNAPSHOTS_PATH}/${id}/follow-ups?limit=20&offset=${(pageNumber - 1) * 20}`, { signal }
  ), id, pageNumber);
}

function sameSession(session: AuthSession) {
  const current = authStore.getSession('admin');
  if (current?.generation !== session.generation || current.subject !== session.subject) {
    throw new Error('管理员会话已变化，请重新打开对账页面');
  }
}
function intentStore() {
  // Persistence must be available before sending a command with an uncertain outcome.
  const storage = window.sessionStorage;
  storage.setItem('reconciliation-storage-probe', '1');
  storage.removeItem('reconciliation-storage-probe');
  return new FinancialCommandIntentStore({ prefix: 'reconciliation', storage });
}

export async function captureSnapshot(session: AuthSession, assetId: number, reason: string) {
  sameSession(session);
  const values = { asset_id: assetId, reason: reason.trim() };
  if (!text(values.reason, 500)) throw new Error('操作原因须为 1–500 字');
  return runRecoverableFinancialCommand({
    store: intentStore(), scope: financialCommandScopeFromSession(session, 'reconciliation-capture', assetId, assetId), values,
    request: async (idempotencyKey) => {
      sameSession(session);
      const receipt = parseSnapshot(await apiRequest<unknown>(SNAPSHOTS_PATH, {
        method: 'POST', body: JSON.stringify({ ...values, idempotency_key: idempotencyKey })
      }), undefined, assetId);
      sameSession(session);
      if (`admin:${receipt.admin_id}` !== session.subject || receipt.reason !== values.reason
        || receipt.idempotency_key !== idempotencyKey) return invalid();
      return receipt;
    }
  });
}

export async function appendSnapshotFollowup(
  session: AuthSession, snapshot: SnapshotSummary, draft: FollowupDraft, reason: string
) {
  sameSession(session);
  const values = { ...draft, notes: draft.notes.trim(), reason: reason.trim() };
  if (!text(values.reason, 500) || !text(values.notes, 2000)) throw new Error('请填写有效的跟进备注及操作原因');
  return runRecoverableFinancialCommand({
    store: intentStore(),
    scope: financialCommandScopeFromSession(session, 'reconciliation-followup', snapshot.id, snapshot.asset_id), values,
    request: async (idempotencyKey) => {
      sameSession(session);
      const receipt = parseFollowup(await apiRequest<unknown>(`${SNAPSHOTS_PATH}/${snapshot.id}/follow-ups`, {
        method: 'POST', body: JSON.stringify({ ...values, idempotency_key: idempotencyKey })
      }), snapshot.id);
      sameSession(session);
      if (`admin:${receipt.admin_id}` !== session.subject || receipt.idempotency_key !== idempotencyKey
        || receipt.reason !== values.reason || receipt.notes !== values.notes
        || receipt.owner_admin_id !== values.owner_admin_id || receipt.due_at !== values.due_at
        || receipt.version !== values.expected_version + 1) return invalid();
      return receipt;
    }
  });
}
