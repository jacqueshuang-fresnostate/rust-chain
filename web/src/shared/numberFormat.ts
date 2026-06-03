import numeral from 'numeral';

export const ADMIN_NUMBER_FORMAT = '0,0.00[0000]';

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

export function formatAdminNumber(value: number | string | null | undefined): string | null {
  if (value === null || value === undefined || value === '') {
    return null;
  }

  const numericValue = numeral(value).value();
  if (numericValue === null || !Number.isFinite(numericValue)) {
    return null;
  }

  return numeral(numericValue).format(ADMIN_NUMBER_FORMAT);
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
