import {
  decimalAbsolute,
  decimalCompare,
  decimalNegate,
  decimalRoundHalfUp,
  decimalSign,
  decimalTextFromBoundary,
  formatDecimalText,
  normalizeDecimalText,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'

export interface FinancialAmountDisplayOptions {
  assetSymbol?: string | null
  maximumFractionDigits?: number
  minimumFractionDigits?: number
  precisionScale?: number | null
  unavailable?: string
  useGrouping?: boolean
}

const ZERO_FRACTION_ASSETS = new Set(['JPY', 'KRW', 'VND'])
const TWO_FRACTION_ASSETS = new Set([
  'USDT', 'USDC', 'USD', 'CNY', 'CNH', 'HKD', 'EUR', 'GBP', 'AUD', 'CAD', 'CHF', 'SGD',
])

export const DEFAULT_FINANCIAL_DISPLAY_DIGITS = 8
export const GENERIC_FINANCIAL_DISPLAY_DIGITS = 6

export function financialAssetDisplayDigits(
  assetSymbol?: string | null,
  precisionScale?: number | null,
): number {
  const symbol = assetSymbol?.trim().toUpperCase() || ''
  const cap = ZERO_FRACTION_ASSETS.has(symbol)
    ? 0
    : TWO_FRACTION_ASSETS.has(symbol)
      ? 2
      : DEFAULT_FINANCIAL_DISPLAY_DIGITS
  if (precisionScale === undefined || precisionScale === null) return cap
  if (!Number.isSafeInteger(precisionScale) || precisionScale < 0 || precisionScale > 18) {
    throw new RangeError('invalid financial display precision')
  }
  return Math.min(cap, precisionScale)
}

export function formatFinancialAmount(
  value: DecimalBoundary,
  locale: string,
  options: FinancialAmountDisplayOptions = {},
): string {
  const normalized = decimalTextFromBoundary(value)
  if (!normalized) return options.unavailable ?? '--'

  const assetDigits = financialAssetDisplayDigits(options.assetSymbol, options.precisionScale)
  const requestedMaximum = options.maximumFractionDigits ?? assetDigits
  if (!Number.isSafeInteger(requestedMaximum) || requestedMaximum < 0 || requestedMaximum > 18) {
    throw new RangeError('invalid financial display maximum')
  }
  const maximum = Math.min(assetDigits, requestedMaximum)
  const minimum = options.minimumFractionDigits ?? 0
  if (!Number.isSafeInteger(minimum) || minimum < 0 || minimum > maximum) {
    throw new RangeError('invalid financial display minimum')
  }

  const threshold = smallestVisibleUnit(maximum)
  const sign = decimalSign(normalized)
  if (sign !== 0 && decimalCompare(decimalAbsolute(normalized), threshold) < 0) {
    const thresholdText = formatDecimalText(threshold, locale, {
      maximumFractionDigits: maximum,
      minimumFractionDigits: maximum,
      preserveNonZero: false,
      useGrouping: options.useGrouping,
    })
    if (sign > 0) return `<${thresholdText}`
    const negativeThreshold = formatDecimalText(decimalNegate(threshold), locale, {
      maximumFractionDigits: maximum,
      minimumFractionDigits: maximum,
      preserveNonZero: false,
      useGrouping: options.useGrouping,
    })
    return `>${negativeThreshold}`
  }

  return formatDecimalText(decimalRoundHalfUp(normalized, maximum), locale, {
    maximumFractionDigits: maximum,
    minimumFractionDigits: minimum,
    preserveNonZero: false,
    useGrouping: options.useGrouping,
  })
}

function smallestVisibleUnit(scale: number): DecimalText {
  if (scale === 0) return normalizeDecimalText('1')
  return normalizeDecimalText(`0.${'0'.repeat(scale - 1)}1`)
}
