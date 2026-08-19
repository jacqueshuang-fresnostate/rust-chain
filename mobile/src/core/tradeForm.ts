export interface BalancePercentageInput {
  available: number
  maximum?: number | null
  mode: 'spot' | 'contract'
  percentage: number
  price: number
  side: 'buy' | 'sell'
}

export interface MarginProductMarginLimits {
  minMargin: number
  maxMargin: number | null
}

export type MarginAmountValidationError = 'invalid' | 'below-minimum' | 'above-maximum'

export interface MarginAmountValidation {
  isValid: boolean
  error: MarginAmountValidationError | null
  minMargin: number
  maxMargin: number | null
}

export type MarginLimitPriceValidationError = 'required' | 'invalid' | 'precision' | 'precision-unavailable'

export interface MarginLimitPriceValidation {
  isValid: boolean
  error: MarginLimitPriceValidationError | null
  value: number | null
  normalized: string | null
  pricePrecision: number | null
}

/** Validates a positive plain decimal without rounding away the user's limit intent. */
export function validateMarginLimitPrice(input: {
  price: string
  pricePrecision?: number | null
}): MarginLimitPriceValidation {
  const draft = input.price.trim()
  const pricePrecision = Number.isInteger(input.pricePrecision) && Number(input.pricePrecision) >= 0
    ? Number(input.pricePrecision)
    : null
  if (!draft) {
    return { isValid: false, error: 'required', value: null, normalized: null, pricePrecision }
  }
  // A trailing decimal point is a valid editing state but not a stable API decimal.
  // Prefix-dot values are canonicalized so `.5` is frozen and sent as `0.5`.
  if (!/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(draft)) {
    return { isValid: false, error: 'invalid', value: null, normalized: null, pricePrecision }
  }
  const normalized = draft.startsWith('.') ? `0${draft}` : draft
  const value = Number(normalized)
  if (!Number.isFinite(value) || value <= 0) {
    return { isValid: false, error: 'invalid', value: null, normalized: null, pricePrecision }
  }
  if (pricePrecision === null) {
    return { isValid: false, error: 'precision-unavailable', value, normalized, pricePrecision }
  }
  const fractionalDigits = (normalized.split('.')[1] || '').replace(/0+$/, '').length
  if (fractionalDigits > pricePrecision) {
    return { isValid: false, error: 'precision', value, normalized, pricePrecision }
  }
  return { isValid: true, error: null, value, normalized, pricePrecision }
}

/**
 * Maps the backend decimal limits without turning a missing maximum into zero.
 * Product configuration guarantees a positive minimum, while a malformed
 * optional maximum is treated the same as an omitted product cap.
 */
export function mapMarginProductMarginLimits(input: {
  min_margin?: unknown
  max_margin?: unknown
}): MarginProductMarginLimits {
  return {
    minMargin: positiveFiniteNumber(input.min_margin) ?? 0,
    maxMargin: positiveFiniteNumber(input.max_margin),
  }
}

/** Returns the real contract shortcut base after applying the product cap. */
export function marginShortcutAvailable(available: number, maximum?: number | null): number {
  if (!Number.isFinite(available) || available <= 0) return 0
  const normalizedMaximum = positiveFiniteNumber(maximum)
  return normalizedMaximum === null ? available : Math.min(available, normalizedMaximum)
}

/** Keeps display rounding from crossing the wallet or product authority. */
export function clampMarginShortcutAmount(
  amount: number,
  available: number,
  maximum?: number | null,
): number {
  if (!Number.isFinite(amount) || amount <= 0) return 0
  return Math.min(amount, marginShortcutAvailable(available, maximum))
}

/** One financial boundary used by field feedback, review, and submission. */
export function validateMarginAmount(input: {
  amount: number
  minMargin: number
  maxMargin?: number | null
}): MarginAmountValidation {
  const minMargin = positiveFiniteNumber(input.minMargin) ?? 0
  const maxMargin = positiveFiniteNumber(input.maxMargin)
  if (!Number.isFinite(input.amount) || input.amount <= 0) {
    return { isValid: false, error: 'invalid', minMargin, maxMargin }
  }
  if (input.amount < minMargin) {
    return { isValid: false, error: 'below-minimum', minMargin, maxMargin }
  }
  if (maxMargin !== null && input.amount > maxMargin) {
    return { isValid: false, error: 'above-maximum', minMargin, maxMargin }
  }
  return { isValid: true, error: null, minMargin, maxMargin }
}

export function quantityForBalancePercentage(input: BalancePercentageInput): number {
  if (!Number.isFinite(input.percentage) || input.percentage <= 0) return 0

  const percentage = Math.min(input.percentage, 1)
  const available = input.mode === 'contract'
    ? marginShortcutAvailable(input.available, input.maximum)
    : marginShortcutAvailable(input.available)
  const budget = available * percentage
  if (input.mode === 'contract' || input.side === 'sell') return budget
  if (!Number.isFinite(input.price) || input.price <= 0) return 0
  return budget / input.price
}

function positiveFiniteNumber(value: unknown): number | null {
  if (typeof value !== 'number' && typeof value !== 'string') return null
  const normalized = typeof value === 'string' ? value.trim() : value
  if (typeof normalized === 'string' && !/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(normalized)) return null
  const parsed = typeof normalized === 'number' ? normalized : Number(normalized)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}
