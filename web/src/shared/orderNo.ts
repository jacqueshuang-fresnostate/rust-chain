import type { ApiRecord } from '../api/types';

function valueText(value: unknown): string {
  if (typeof value === 'string') {
    return value.trim();
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return String(value);
  }
  return '';
}

function timestampToken(value: unknown): string {
  const raw = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : NaN;
  if (!Number.isFinite(raw) || raw <= 0) {
    return '00000000';
  }
  const millis = raw < 10_000_000_000 ? raw * 1000 : raw;
  const date = new Date(millis);
  if (Number.isNaN(date.getTime())) {
    return '00000000';
  }
  const year = String(date.getFullYear()).padStart(4, '0');
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}${month}${day}`;
}

function idToken(value: unknown): string {
  const text = valueText(value);
  if (!text) {
    return '000000';
  }
  const numeric = Number(text);
  if (Number.isInteger(numeric) && numeric >= 0) {
    return numeric.toString(36).toUpperCase().padStart(6, '0').slice(-6);
  }
  let hash = 0;
  for (const char of text) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return hash.toString(36).toUpperCase().padStart(6, '0').slice(-6);
}

export function formatBusinessOrderNo(prefix: string, record: ApiRecord): string {
  const existing = valueText(record.order_no ?? record.orderNo);
  if (existing) {
    return existing;
  }
  const datePart = timestampToken(record.created_at ?? record.subscribed_at ?? record.createTime ?? record.createdAt);
  return `${prefix}${datePart}${idToken(record.id)}`;
}
