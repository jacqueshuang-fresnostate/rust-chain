import {
  nullableDecimalText,
  requiredDecimalText,
  type DecimalText,
} from './decimal.ts'

const ASSET_SYMBOL_PATTERN = /^[A-Z0-9]{1,32}$/

export function requiredRealizedReturnDecimal(
  value: unknown,
  field: string,
  contract: string,
): DecimalText {
  return requiredDecimalText(value, field, contract, {
    maxIntegerDigits: 20,
    maxScale: 18,
  })
}

export function nullableRealizedReturnDecimal(
  value: unknown,
  field: string,
  contract: string,
): DecimalText | null {
  return nullableDecimalText(value, field, contract, {
    maxIntegerDigits: 20,
    maxScale: 18,
  })
}

export function normalizeRealizedReturnTimestamp(
  value: unknown,
  field: string,
  contract: string,
): number {
  const parsed = typeof value === 'number'
    ? value
    : typeof value === 'string' && /^\d+$/.test(value.trim())
      ? Number(value.trim())
      : Number.NaN
  if (parsed <= 0 || !Number.isSafeInteger(parsed)) {
    throw new Error(`invalid ${contract} ${field}`)
  }
  const normalized = parsed < 1_000_000_000_000 ? parsed * 1000 : parsed
  if (!Number.isSafeInteger(normalized)) throw new Error(`invalid ${contract} ${field}`)
  return normalized
}

export function normalizeRealizedReturnAssetSymbol(
  value: unknown,
  field: string,
  contract: string,
): string {
  if (typeof value !== 'string') throw new Error(`invalid ${contract} ${field}`)
  const normalized = value.trim().toUpperCase()
  if (!ASSET_SYMBOL_PATTERN.test(normalized)) throw new Error(`invalid ${contract} ${field}`)
  return normalized
}

export function normalizeRealizedReturnAssetSymbols(
  value: unknown,
  field: string,
  contract: string,
): string[] {
  if (!Array.isArray(value)) throw new Error(`invalid ${contract} ${field}`)
  return [...new Set(value.map((asset) => (
    normalizeRealizedReturnAssetSymbol(asset, field, contract)
  )))]
}
