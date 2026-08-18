import type { AdminAuditLog } from './auditApi';
import {
  auditActionLabel,
  auditTargetHref,
  auditTargetLabel,
  buildAuditFieldChanges,
  redactAuditFreeText
} from './auditPresentation';

export const AUDIT_LOGS_EXPORT_FILENAME = 'HIPPO-审计日志-当前结果.csv';

function csvCell(value: unknown): string {
  if (value === null || value === undefined) {
    return '';
  }
  // Every cell, including metadata such as action codes, object IDs and trace IDs,
  // passes through the same free-text masker as the visible audit detail.
  const text = redactAuditFreeText(String(value));
  // Prevent spreadsheet formula execution when an administrator opens the CSV.
  const spreadsheetSafeText = /^[\t ]*[=+@-]/u.test(text) ? `'${text}` : text;
  return /[",\r\n]/u.test(spreadsheetSafeText)
    ? `"${spreadsheetSafeText.replace(/"/gu, '""')}"`
    : spreadsheetSafeText;
}

function safeReason(reason: string | null): string {
  if (!reason?.trim()) {
    return '未填写原因';
  }
  return redactAuditFreeText(reason).trim();
}

function auditChangesText(log: AdminAuditLog): string {
  const changes = buildAuditFieldChanges(log.before_json, log.after_json);
  if (changes.length === 0) {
    return log.before_json === null && log.after_json === null
      ? '未记录前后快照'
      : '前后快照一致，无字段变化';
  }
  return changes
    .map((change) => `${change.label}：${change.before} → ${change.after}`)
    .join('；');
}

function exportTimestamp(value: number): string {
  const date = new Date(value);
  return Number.isFinite(value) && Number.isFinite(date.getTime()) ? date.toISOString() : '';
}

export function toAuditLogsCsv(logs: AdminAuditLog[]): string {
  const header = [
    '日志 ID',
    '发生时间',
    '管理员',
    '中文动作',
    '动作代码',
    '对象类型',
    '对象 ID',
    '对象页面',
    '操作原因',
    '来源 IP',
    'Request ID',
    '字段差异'
  ];
  const rows = logs.map((log) => [
    log.id,
    exportTimestamp(log.created_at),
    `管理员 #${log.admin_id}`,
    auditActionLabel(log.action, log.target_type),
    log.action,
    auditTargetLabel(log.target_type),
    log.target_id,
    auditTargetHref(log.target_type) ?? '',
    safeReason(log.reason),
    log.ip?.trim() || '未记录',
    log.request_id?.trim() || '未记录',
    auditChangesText(log)
  ]);

  return [header, ...rows].map((row) => row.map(csvCell).join(',')).join('\n');
}

export function downloadCurrentAuditLogs(logs: AdminAuditLog[]): void {
  const url = URL.createObjectURL(
    new Blob([`\uFEFF${toAuditLogsCsv(logs)}`], { type: 'text/csv;charset=utf-8' })
  );
  const link = document.createElement('a');
  link.href = url;
  link.download = AUDIT_LOGS_EXPORT_FILENAME;
  link.click();
  URL.revokeObjectURL(url);
}
