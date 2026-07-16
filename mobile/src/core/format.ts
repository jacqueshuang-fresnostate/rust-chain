import { currentRuntimeIntlLocale } from './runtimeLocale.ts'

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
  const numberValue = asNumber(value)
  if (numberValue === 0) return '0.00'
  const digits = numberValue < 0.1 ? 6 : numberValue < 1 ? 4 : 2
  return new Intl.NumberFormat(currentRuntimeIntlLocale(), { maximumFractionDigits: digits, minimumFractionDigits: digits }).format(numberValue)
}

export function formatAmount(value: unknown, digits = 4): string {
  return new Intl.NumberFormat(currentRuntimeIntlLocale(), { maximumFractionDigits: digits }).format(asNumber(value))
}

export function formatFiat(value: unknown, currency = 'USD'): string {
  return new Intl.NumberFormat(currentRuntimeIntlLocale(), {
    style: 'currency',
    currency,
    maximumFractionDigits: 2,
  }).format(asNumber(value))
}

export function formatPercent(value: unknown): string {
  const numberValue = asNumber(value)
  return `${numberValue > 0 ? '+' : ''}${numberValue.toFixed(2)}%`
}

export function formatCompact(value: unknown): string {
  return new Intl.NumberFormat(currentRuntimeIntlLocale(), {
    notation: 'compact',
    maximumFractionDigits: 2,
  }).format(asNumber(value))
}

export function shortAddress(value: string, leading = 8, trailing = 6): string {
  if (value.length <= leading + trailing + 3) return value
  return `${value.slice(0, leading)}...${value.slice(-trailing)}`
}

export function formatDateTime(value: unknown): string {
  const timestamp = asNumber(value)
  if (!timestamp) return '--'
  const normalized = timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
  return new Intl.DateTimeFormat(currentRuntimeIntlLocale(), {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(normalized)).replace(/\//g, '-')
}
