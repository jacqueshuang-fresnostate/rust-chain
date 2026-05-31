import { apiRequest } from './client';
import type { ApiRecord } from './types';

export type AdminResourceFilters = Record<string, string | number | boolean | null | undefined>;

export type AdminResourceListResult<T extends ApiRecord = ApiRecord> = {
  rows: T[];
  raw: ApiRecord;
};

function appendQuery(endpoint: string, filters: AdminResourceFilters) {
  const params = new URLSearchParams();

  Object.entries(filters).forEach(([key, value]) => {
    if (value === null || value === undefined || value === '') {
      return;
    }

    params.set(key, String(value));
  });

  const query = params.toString();
  if (!query) {
    return endpoint;
  }

  return `${endpoint}${endpoint.includes('?') ? '&' : '?'}${query}`;
}

function isApiRecordArray(value: unknown): value is ApiRecord[] {
  return Array.isArray(value) && value.every((item) => item !== null && typeof item === 'object' && !Array.isArray(item));
}

export async function listAdminResource<T extends ApiRecord = ApiRecord>(
  endpoint: string,
  responseKey: string,
  filters: AdminResourceFilters = {}
): Promise<AdminResourceListResult<T>> {
  const raw = await apiRequest<ApiRecord>(appendQuery(endpoint, filters));
  const rowsValue = raw[responseKey];

  return {
    rows: isApiRecordArray(rowsValue) ? (rowsValue as T[]) : [],
    raw
  };
}
