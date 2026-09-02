import { formatDecimalText, type DecimalFormatOptions } from './decimal';

export const ADMIN_NUMBER_FORMAT = '0,0.00[0000]';
export const ADMIN_GENERIC_MAXIMUM_FRACTION_DIGITS = 6;
export const ADMIN_ASSET_MAXIMUM_FRACTION_DIGITS = 8;

const zeroFractionAssets = new Set(['JPY', 'KRW', 'VND']);
const twoFractionAssets = new Set([
  'USDT', 'USDC', 'USD', 'CNY', 'CNH', 'HKD', 'EUR', 'GBP', 'AUD', 'CAD', 'CHF', 'SGD'
]);

export type AdminAmountFormatOptions = {
  asset?: string;
  precision?: number;
};

const excludedNumericKeyParts = [
  'id',
  'time',
  'timestamp',
  'date',
  'version',
  'precision',
  'scale',
  'level',
  'days',
  'seconds',
  'minutes',
  'hours',
  'interval',
  'duration',
  'port'
];

const includedNumericKeyParts = [
  'amount',
  'price',
  'quantity',
  'qty',
  'balance',
  'available',
  'frozen',
  'locked',
  'rate',
  'ratio',
  'fee',
  'interest',
  'margin',
  'volume',
  'total',
  'count',
  'size',
  'value',
  'pnl',
  'profit',
  'loss',
  'income',
  'revenue',
  'cost'
];

export function formatAdminNumber(value: number | string | null | undefined, options: DecimalFormatOptions = {}): string | null {
  if (value === null || value === undefined || value === '') {
    return null;
  }
  return formatDecimalText(value, {
    maximumFractionDigits: ADMIN_GENERIC_MAXIMUM_FRACTION_DIGITS,
    ...options
  });
}

export function adminAssetDisplayDigits(asset?: string, precision?: number): number | null {
  if (precision !== undefined && (!Number.isInteger(precision) || precision < 0 || precision > 18)) {
    return null;
  }
  const normalized = asset?.trim().toUpperCase() ?? '';
  const assetMaximum = zeroFractionAssets.has(normalized)
    ? 0
    : twoFractionAssets.has(normalized)
      ? 2
      : normalized
        ? ADMIN_ASSET_MAXIMUM_FRACTION_DIGITS
        : ADMIN_GENERIC_MAXIMUM_FRACTION_DIGITS;
  return Math.min(assetMaximum, precision ?? assetMaximum);
}

/**
 * 后台金额展示策略。原始十进制字符串仍保留在响应、表单和 CSV 中，只有文本渲染会舍入。
 */
export function formatAdminAmount(
  value: number | string | null | undefined,
  options: AdminAmountFormatOptions = {}
): string | null {
  if (value === null || value === undefined || value === '') return null;
  const maximumFractionDigits = adminAssetDisplayDigits(options.asset, options.precision);
  if (maximumFractionDigits === null) return null;
  return formatDecimalText(value, {
    maximumFractionDigits,
    minimumFractionDigits: Math.min(2, maximumFractionDigits),
    precision: options.precision,
    preserveNonZero: true
  });
}

export function shouldFormatAdminNumericKey(key: string): boolean {
  const normalized = key.toLowerCase();
  if (excludedNumericKeyParts.some((part) => normalized === part || normalized.endsWith(`_${part}`) || normalized.includes(`_${part}_`))) {
    return false;
  }

  return includedNumericKeyParts.some((part) => normalized === part || normalized.endsWith(`_${part}`) || normalized.includes(`_${part}_`));
}

export function formatAdminDisplayValue(key: string, value: unknown): string | null {
  if ((typeof value !== 'number' && typeof value !== 'string') || !shouldFormatAdminNumericKey(key)) {
    return null;
  }

  return formatAdminNumber(value);
}
