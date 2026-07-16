import { asNumber, splitSymbol } from './format.ts'
import type { MarketTicker } from './types.ts'

export interface BackendMarketRecord {
  id?: string | number | null
  symbol: string
  logo_url?: string | null
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
  observed_at?: number | null
}

export function mapMarketTicker(market: BackendMarketRecord, ticker: BackendTickerRecord): MarketTicker {
  const lastPrice = asNumber(ticker.last_price)
  const priceChange = asNumber(ticker.price_change_24h)
  const openPrice = asNumber(ticker.open_24h, lastPrice - priceChange)
  const pair = splitSymbol(market.symbol || ticker.symbol || '', market.base_asset, market.quote_asset)
  const observedAt = normalizeTimestamp(ticker.observed_at)

  return {
    id: asNumber(market.id) || undefined,
    symbol: `${pair.base}/${pair.quote}`,
    base: pair.base,
    quote: pair.quote,
    iconUrl: market.logo_url?.trim() || undefined,
    lastPrice,
    openPrice,
    highPrice: asNumber(ticker.high_24h, lastPrice),
    lowPrice: asNumber(ticker.low_24h, lastPrice),
    volume: asNumber(ticker.volume_24h),
    changePercent: openPrice ? ((lastPrice - openPrice) / openPrice) * 100 : 0,
    observedAt,
  }
}

function normalizeTimestamp(value: unknown): number {
  const time = asNumber(value)
  return time > 0 && time < 1_000_000_000_000 ? time * 1000 : time
}
