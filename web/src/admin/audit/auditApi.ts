import { apiRequest } from '../../api/client';

export const AUDIT_LOGS_ENDPOINT = '/admin/api/v1/audit-logs';

export type AdminAuditLog = {
  action: string;
  admin_id: number;
  after_json: unknown | null;
  before_json: unknown | null;
  created_at: number;
  id: number;
  ip: string | null;
  reason: string | null;
  request_id: string | null;
  target_id: string;
  target_type: string;
};

export type AdminAuditLogsResponse = {
  logs: AdminAuditLog[];
  total: number;
};

export type AdminAuditLogsQuery = {
  action?: string;
  admin_id?: string;
  created_from?: number;
  created_to?: number;
  limit: number;
  offset: number;
  target_id?: string;
  target_type?: string;
};

export function buildAdminAuditLogsPath(query: AdminAuditLogsQuery): string {
  const params = new URLSearchParams();
  if (query.admin_id) {
    params.set('admin_id', query.admin_id);
  }
  if (query.action) {
    params.set('action', query.action);
  }
  if (query.target_type) {
    params.set('target_type', query.target_type);
  }
  if (query.target_id) {
    params.set('target_id', query.target_id);
  }
  if (query.created_from !== undefined) {
    params.set('created_from', String(query.created_from));
  }
  if (query.created_to !== undefined) {
    params.set('created_to', String(query.created_to));
  }
  params.set('limit', String(query.limit));
  params.set('offset', String(query.offset));
  return `${AUDIT_LOGS_ENDPOINT}?${params.toString()}`;
}

export function localDateTimeToUnixMillis(value: string): number | undefined {
  if (!value.trim()) {
    return undefined;
  }
  const milliseconds = new Date(value).getTime();
  return Number.isFinite(milliseconds) ? milliseconds : undefined;
}

export function getAdminAuditLogs(query: AdminAuditLogsQuery): Promise<AdminAuditLogsResponse> {
  return apiRequest<AdminAuditLogsResponse>(buildAdminAuditLogsPath(query));
}
