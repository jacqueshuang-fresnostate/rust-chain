import {
  decimalCompare,
  decimalDivide,
  decimalMinimum,
  decimalMultiply,
  decimalPortion,
  decimalTextFromBoundary,
  decimalTextFromFiniteNumber,
  decimalTruncate,
  decimalWithinRange,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'

export interface BalancePercentageInput {
  available: DecimalBoundary
  maximum?: DecimalBoundary
  mode: 'spot' | 'contract'
  percentage: number
  price: DecimalBoundary
  side: 'buy' | 'sell'
}

export interface BalancePercentagePointsInput extends Omit<BalancePercentageInput, 'percentage'> {
  /** Integer 0..100 value from the range control. */
  percentagePoints: number
}

export interface MarginProductMarginLimits {
  minMargin: number
  maxMargin: number | null
  minMarginText: DecimalText
  maxMarginText: DecimalText | null
}

export type MarginAmountValidationError = 'invalid' | 'below-minimum' | 'above-maximum'

export interface MarginAmountValidation {
  isValid: boolean
  error: MarginAmountValidationError | null
  minMargin: DecimalText
  maxMargin: DecimalText | null
}

export type MarginLimitPriceValidationError = 'required' | 'invalid' | 'precision' | 'precision-unavailable'

export interface MarginLimitPriceValidation {
  isValid: boolean
  error: MarginLimitPriceValidationError | null
  value: DecimalText | null
  normalized: DecimalText | null
  pricePrecision: number | null
}

/** Validates a positive plain decimal without rounding away the user's limit intent. */
export function validateMarginLimitPrice(input: {
  price: string
  pricePrecision?: number | null
}): MarginLimitPriceValidation {
  const draft = input.price.trim()
  const pricePrecision = typeof input.pricePrecision === 'number'
    && Number.isInteger(input.pricePrecision)
    && input.pricePrecision >= 0
    ? input.pricePrecision
    : null
  if (!draft) return { isValid: false, error: 'required', value: null, normalized: null, pricePrecision }
  if (draft.endsWith('.')) return { isValid: false, error: 'invalid', value: null, normalized: null, pricePrecision }
  const normalized = positiveDecimalInput(draft, 100)
  if (!normalized) return { isValid: false, error: 'invalid', value: null, normalized: null, pricePrecision }
  if (pricePrecision === null) {
    return { isValid: false, error: 'precision-unavailable', value: normalized, normalized, pricePrecision }
  }
  const fractionalDigits = (normalized.split('.')[1] || '').length
  if (fractionalDigits > pricePrecision) {
    return { isValid: false, error: 'precision', value: normalized, normalized, pricePrecision }
  }
  return { isValid: true, error: null, value: normalized, normalized, pricePrecision }
}

/** Maps exact backend limits while retaining number fields only for legacy display surfaces. */
export function mapMarginProductMarginLimits(input: {
  min_margin?: unknown
  max_margin?: unknown
}): MarginProductMarginLimits {
  const minMarginText = positiveBoundary(input.min_margin) || normalizeDecimalText('0')
  const maxMarginText = positiveBoundary(input.max_margin)
  return {
    minMargin: legacyDisplayNumber(minMarginText),
    maxMargin: maxMarginText ? legacyDisplayNumber(maxMarginText) : null,
    minMarginText,
    maxMarginText,
  }
}

/** Returns the real contract shortcut base after applying the product cap. */
export function marginShortcutAvailable(
  available: DecimalBoundary,
  maximum?: DecimalBoundary,
): DecimalText {
  const availableText = positiveBoundary(available)
  if (!availableText) return normalizeDecimalText('0')
  const maximumText = positiveBoundary(maximum)
  return maximumText ? decimalMinimum(availableText, maximumText) || availableText : availableText
}

/** Keeps a derived shortcut from crossing the wallet or product authority. */
export function clampMarginShortcutAmount(
  amount: DecimalBoundary,
  available: DecimalBoundary,
  maximum?: DecimalBoundary,
): DecimalText {
  const amountText = positiveBoundary(amount)
  if (!amountText) return normalizeDecimalText('0')
  return decimalMinimum(amountText, marginShortcutAvailable(available, maximum)) || normalizeDecimalText('0')
}

/** One exact financial boundary used by field feedback, review, and submission. */
export function validateMarginAmount(input: {
  amount: string | DecimalText
  minMargin: DecimalBoundary
  maxMargin?: DecimalBoundary
}): MarginAmountValidation {
  const amount = positiveDecimalInput(input.amount)
  const minMargin = positiveBoundary(input.minMargin) || normalizeDecimalText('0')
  const maxMargin = positiveBoundary(input.maxMargin)
  if (!amount) return { isValid: false, error: 'invalid', minMargin, maxMargin }
  if (decimalCompare(amount, minMargin) < 0) {
    return { isValid: false, error: 'below-minimum', minMargin, maxMargin }
  }
  if (maxMargin && decimalCompare(amount, maxMargin) > 0) {
    return { isValid: false, error: 'above-maximum', minMargin, maxMargin }
  }
  return { isValid: true, error: null, minMargin, maxMargin }
}

export function quantityForBalancePercentage(input: BalancePercentageInput): DecimalText {
  if (!Number.isFinite(input.percentage) || input.percentage <= 0) return normalizeDecimalText('0')
  const ratio = decimalTextFromFiniteNumber(Math.min(input.percentage, 1))
  const available = input.mode === 'contract'
    ? marginShortcutAvailable(input.available, input.maximum)
    : marginShortcutAvailable(input.available)
  const budget = decimalTruncate(decimalMultiply(available, ratio), 18)
  if (input.mode === 'contract' || input.side === 'sell') return budget
  const price = positiveBoundary(input.price)
  return price ? decimalDivide(budget, price, 18) : normalizeDecimalText('0')
}

/**
 * Applies an integer UI percentage through bigint DecimalText division. This is
 * the production form path; the ratio-based overload above remains for older callers.
 */
export function quantityForBalancePercentagePoints(
  input: BalancePercentagePointsInput,
): DecimalText {
  if (!Number.isSafeInteger(input.percentagePoints) || input.percentagePoints <= 0) {
    return normalizeDecimalText('0')
  }
  const percentagePoints = Math.min(input.percentagePoints, 100)
  const available = input.mode === 'contract'
    ? marginShortcutAvailable(input.available, input.maximum)
    : marginShortcutAvailable(input.available)
  const budget = decimalPortion(available, percentagePoints, 100, 18)
  if (input.mode === 'contract' || input.side === 'sell') return budget
  const price = positiveBoundary(input.price)
  return price ? decimalDivide(budget, price, 18) : normalizeDecimalText('0')
}

export function financialAmountWithin(
  amount: string,
  range: { available?: DecimalBoundary; maximum?: DecimalBoundary; minimum?: DecimalBoundary },
  maxScale = 18,
): DecimalText | null {
  const normalized = positiveDecimalInput(amount, maxScale)
  return decimalWithinRange(normalized, range) ? normalized : null
}

function positiveBoundary(value: unknown): DecimalText | null {
  if (typeof value === 'string' && value.trim().startsWith('+')) return null
  const decimal = decimalTextFromBoundary(value as DecimalBoundary, { allowNegative: false })
  return decimal && decimalCompare(decimal, normalizeDecimalText('0')) > 0 ? decimal : null
}

function legacyDisplayNumber(value: DecimalText): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}
