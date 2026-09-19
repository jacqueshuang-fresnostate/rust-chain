import { currentRuntimeIntlLocale } from './runtimeLocale.ts'
import { decimalAbsolute, decimalCompare, decimalMultiply, decimalRoundHalfUp, decimalSign, decimalTextFromBoundary, formatDecimalText, normalizeDecimalText, type DecimalBoundary, type DecimalText } from './decimal.ts'
import { normalizeTimestamp } from './numeric.ts'

export function asNumber(value: unknown, fallback = 0): number {
  const numberValue = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(numberValue) ? numberValue : fallback
}

export function normalizeSymbol(symbol: string): string {
  return symbol.replace(/[-_/\s]/g, '').toUpperCase()
}

export function splitSymbol(symbol: string, baseAsset?: string, quoteAsset?: string): { base: string; quote: string } {
  if (baseAsset && quoteAsset) return { base: baseAsset.toUpperCase(), quote: quoteAsset.toUpperCase() }

  const normalized = symbol.trim().toUpperCase()
  const separated = normalized.split(/[\/_-]/).filter(Boolean)
  if (separated.length >= 2) return { base: separated[0], quote: separated[1] }

  const quotes = ['USDT', 'USDC', 'BTC', 'ETH', 'USD']
  const quote = quotes.find((candidate) => normalized.endsWith(candidate)) || 'USDT'
  const base = normalized.slice(0, Math.max(0, normalized.length - quote.length)) || normalized
  return { base, quote }
}

export function formatPrice(value: unknown): string {
  const decimal = decimalTextFromBoundary(value as DecimalBoundary)
  if (!decimal) return '--'
  const absolute = decimalAbsolute(decimal)
  const digits = decimalSign(decimal) === 0 ? 2 : decimalCompare(absolute, normalizeDecimalText('0.1')) < 0 ? 6 : decimalCompare(absolute, normalizeDecimalText('1')) < 0 ? 4 : 2
  return formatDecimalText(roundVisible(decimal, digits), currentRuntimeIntlLocale(), { maximumFractionDigits: digits, minimumFractionDigits: digits })
}

export function formatAmount(value: unknown, digits = 4): string {
  const decimal = decimalTextFromBoundary(value as DecimalBoundary)
  return decimal ? formatDecimalText(roundVisible(decimal, digits), currentRuntimeIntlLocale(), { maximumFractionDigits: digits }) : '--'
}

/** Confirmation and bill amounts retain every source digit, including tiny fees. */
export function formatExactAmount(value: unknown): string {
  const decimal = decimalTextFromBoundary(value as DecimalBoundary)
  return decimal ? formatDecimalText(decimal, currentRuntimeIntlLocale()) : '--'
}

export function formatRatePercent(value: DecimalBoundary): string {
  const decimal = decimalTextFromBoundary(value)
  return decimal ? formatAmount(decimalMultiply(decimal, normalizeDecimalText('100')), 2) : '--'
}

export function formatFiat(value: unknown, currency = 'USD'): string {
  const decimal = decimalTextFromBoundary(value as DecimalBoundary)
  if (!decimal) return '--'
  const locale = currentRuntimeIntlLocale()
  const amount = formatDecimalText(decimalAbsolute(roundVisible(decimal, 2)), locale, {
    minimumFractionDigits: 2, maximumFractionDigits: 2,
  })
  return new Intl.NumberFormat(locale, {
    style: 'currency',
    currency,
    maximumFractionDigits: 2,
  }).formatToParts(decimalSign(decimal) < 0 ? -1 : 1).map((part) => {
    if (part.type === 'integer') return amount
    return ['group', 'decimal', 'fraction'].includes(part.type) ? '' : part.value
  }).join('')
}

export function formatPercent(value: unknown): string {
  const decimal = decimalTextFromBoundary(value as DecimalBoundary)
  if (!decimal) return '--'
  const text = formatDecimalText(roundVisible(decimal, 2), currentRuntimeIntlLocale(), {
    minimumFractionDigits: 2, maximumFractionDigits: 2, useGrouping: false,
  })
  return `${decimalSign(decimal) > 0 ? '+' : ''}${text}%`
}

export function formatCompact(value: unknown): string {
  if (value == null || value === '') return '--'
  const approximate = asNumber(value, Number.NaN)
  if (!Number.isFinite(approximate)) return '--'
  return new Intl.NumberFormat(currentRuntimeIntlLocale(), {
    notation: 'compact',
    maximumFractionDigits: 2,
  }).format(approximate)
}

function roundVisible(value: DecimalText, digits: number): DecimalText {
  const rounded = decimalRoundHalfUp(value, digits)
  return decimalSign(value) !== 0 && decimalSign(rounded) === 0 ? value : rounded
}

export function shortAddress(value: string, leading = 8, trailing = 6): string {
  if (value.length <= leading + trailing + 3) return value
  return `${value.slice(0, leading)}...${value.slice(-trailing)}`
}

export function formatDateTime(value: unknown): string {
  let timestamp: number
  try { timestamp = normalizeTimestamp(value) } catch { return '--' }
  if (!timestamp) return '--'
  return new Intl.DateTimeFormat(currentRuntimeIntlLocale(), {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(timestamp)).replace(/\//g, '-')
}
