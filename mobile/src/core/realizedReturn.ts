const DECIMAL_PATTERN = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/
const ASSET_SYMBOL_PATTERN = /^[A-Z0-9]{1,32}$/

export function requiredRealizedReturnNumber(
  value: unknown,
  field: string,
  contract: string,
): number {
  if (typeof value !== 'number' && typeof value !== 'string') {
    throw new Error(`invalid ${contract} ${field}`)
  }
  if (typeof value === 'string' && !value.trim()) {
    throw new Error(`invalid ${contract} ${field}`)
  }
  if (typeof value === 'string' && !DECIMAL_PATTERN.test(value.trim())) {
    throw new Error(`invalid ${contract} ${field}`)
  }
  const parsed = typeof value === 'number' ? value : Number(value.trim())
  if (!Number.isFinite(parsed)) throw new Error(`invalid ${contract} ${field}`)
  return Object.is(parsed, -0) ? 0 : parsed
}

export function nullableRealizedReturnNumber(
  value: unknown,
  field: string,
  contract: string,
): number | null {
  return value === null ? null : requiredRealizedReturnNumber(value, field, contract)
}

export function normalizeRealizedReturnTimestamp(
  value: unknown,
  field: string,
  contract: string,
): number {
  const parsed = requiredRealizedReturnNumber(value, field, contract)
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
