import { asNumber, splitSymbol } from './format.ts'
import { tryNormalizeDecimalText } from './decimal.ts'
import type { MarketTicker } from './types.ts'

export interface BackendMarketRecord {
  id?: string | number | null
  symbol: string
  logo_url?: string | null
  base_logo_url?: string | null
  quote_logo_url?: string | null
  base_asset?: string
  quote_asset?: string
}

export interface BackendTickerRecord {
  symbol?: string
  last_price?: string | number | null
  open_24h?: string | number | null
  high_24h?: string | number | null
  low_24h?: string | number | null
  volume_24h?: string | number | null
  price_change_24h?: string | number | null
  price_change_percent_24h?: string | number | null
  observed_at?: number | null
}

export function mapMarketTicker(market: BackendMarketRecord, ticker: BackendTickerRecord): MarketTicker {
  const lastPrice = asNumber(ticker.last_price)
  const explicitOpenPrice = optionalFiniteNumber(ticker.open_24h)
  const explicitChangePercent = optionalFiniteNumber(ticker.price_change_percent_24h)
  const priceChange = asNumber(ticker.price_change_24h)
  const percentDenominator = explicitChangePercent === null ? 0 : 1 + explicitChangePercent / 100
  const openPriceFromPercent =
    lastPrice > 0 && percentDenominator > 0 ? lastPrice / percentDenominator : null
  const openPrice = explicitOpenPrice ?? openPriceFromPercent ?? lastPrice - priceChange
  const pair = splitSymbol(market.symbol || ticker.symbol || '', market.base_asset, market.quote_asset)
  const observedAt = normalizeTimestamp(ticker.observed_at)

  return {
    id: asNumber(market.id) || undefined,
    symbol: `${pair.base}/${pair.quote}`,
    base: pair.base,
    quote: pair.quote,
    iconUrl: market.logo_url?.trim() || undefined,
    baseIconUrl: market.base_logo_url?.trim() || undefined,
    quoteIconUrl: market.quote_logo_url?.trim() || undefined,
    lastPrice,
    lastPriceText: exactPositiveTickerPrice(ticker.last_price) || undefined,
    openPrice,
    highPrice: asNumber(ticker.high_24h, lastPrice),
    lowPrice: asNumber(ticker.low_24h, lastPrice),
    volume: asNumber(ticker.volume_24h),
    changePercent:
      explicitChangePercent ?? (openPrice ? ((lastPrice - openPrice) / openPrice) * 100 : 0),
    observedAt,
  }
}

function exactPositiveTickerPrice(value: unknown) {
  if (typeof value !== 'string') return null
  return tryNormalizeDecimalText(value, {
    allowNegative: false,
    allowZero: false,
    maxIntegerDigits: 20,
    maxScale: 18,
  })
}

function optionalFiniteNumber(value: unknown): number | null {
  if (
    (typeof value !== 'number' && typeof value !== 'string')
    || (typeof value === 'string' && value.trim() === '')
  ) {
    return null
  }

  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

function normalizeTimestamp(value: unknown): number {
  const time = asNumber(value)
  return time > 0 && time < 1_000_000_000_000 ? time * 1000 : time
}
