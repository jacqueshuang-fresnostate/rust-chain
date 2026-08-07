import { asNumber, normalizeSymbol } from './format.ts'
import type { MarketFavorite } from './types.ts'

export interface BackendMarketFavoriteRecord {
  market_id?: string | number | null
  symbol?: string | null
  logo_url?: string | null
  base_logo_url?: string | null
  quote_logo_url?: string | null
}

export function mapMarketFavorite(record: BackendMarketFavoriteRecord): MarketFavorite | null {
  const marketId = asNumber(record.market_id)
  const symbol = normalizeSymbol(String(record.symbol || ''))
  if (marketId <= 0 || !symbol) return null

  return {
    marketId,
    symbol,
    iconUrl: record.logo_url?.trim() || undefined,
    baseIconUrl: record.base_logo_url?.trim() || undefined,
    quoteIconUrl: record.quote_logo_url?.trim() || undefined,
  }
}
