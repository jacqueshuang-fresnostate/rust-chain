import { apiRequest, ContractError } from './client';
import type { ApiRecord } from './types';
import { canonicalDecimalText } from '../shared/decimal';

export type AdminResourceFilters = Record<string, string | number | boolean | null | undefined>;

export type AdminResourceListResult<T extends ApiRecord = ApiRecord> = {
  rows: T[];
  raw: ApiRecord;
  total?: number;
};

export type AdminResourceRowContract = {
  /** 必须由 DTO 显式携带的字段；值可以按业务合同为 null。 */
  requiredFields?: readonly string[];
  /** 金额/价格/费率字段必须是可解析的 Decimal text，不接受已舍入的 number。 */
  decimalFields?: readonly string[];
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

function validateRows(rows: ApiRecord[], endpoint: string, contract: AdminResourceRowContract | undefined): void {
  if (!contract) return;
  rows.forEach((row, index) => {
    contract.requiredFields?.forEach((field) => {
      if (!Object.prototype.hasOwnProperty.call(row, field)) {
        throw new ContractError(`接口 ${endpoint} 的第 ${index + 1} 行缺少必填字段 ${field}`, { path: endpoint });
      }
    });
    contract.decimalFields?.forEach((field) => {
      const value = row[field];
      if (value === null || value === undefined) return;
      if (typeof value !== 'string' || canonicalDecimalText(value) === null) {
        throw new ContractError(`接口 ${endpoint} 的第 ${index + 1} 行字段 ${field} 必须是 Decimal text`, { path: endpoint });
      }
    });
  });
}

export async function listAdminResource<T extends ApiRecord = ApiRecord>(
  endpoint: string,
  responseKey: string,
  filters: AdminResourceFilters = {},
  options: { rowContract?: AdminResourceRowContract; signal?: AbortSignal } = {}
): Promise<AdminResourceListResult<T>> {
  const value: unknown = await apiRequest<unknown>(appendQuery(endpoint, filters), { signal: options.signal });
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ContractError(`接口 ${endpoint} 的响应必须是对象`, { path: endpoint });
  }
  const raw = value as ApiRecord;
  const rowsValue = raw[responseKey];
  const total = raw.total;

  if (!Object.prototype.hasOwnProperty.call(raw, responseKey)) {
    throw new ContractError(`接口 ${endpoint} 缺少列表字段 ${responseKey}`, { path: endpoint });
  }
  if (!isApiRecordArray(rowsValue)) {
    throw new ContractError(`接口 ${endpoint} 的 ${responseKey} 必须是对象数组`, { path: endpoint });
  }
  if (total !== undefined && (typeof total !== 'number' || !Number.isSafeInteger(total) || total < 0)) {
    throw new ContractError(`接口 ${endpoint} 的 total 必须是非负安全整数`, { path: endpoint });
  }
  validateRows(rowsValue, endpoint, options.rowContract);

  return {
    rows: rowsValue as T[],
    raw,
    total: typeof total === 'number' ? total : undefined
  };
}
