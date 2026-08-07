import { client, requestUrl } from './client'
import {
  mapMarketFavorite,
  type BackendMarketFavoriteRecord,
} from '@/core/marketFavoriteMapper'
import { normalizeSymbol } from '@/core/format'
import type { MarketFavorite } from '@/core/types'

export async function fetchMarketFavorites(): Promise<MarketFavorite[]> {
  const response = await client.get<{ favorites?: BackendMarketFavoriteRecord[] }>(
    requestUrl('/user/market-favorites'),
  )
  return (response.data.favorites || [])
    .map(mapMarketFavorite)
    .filter((favorite): favorite is MarketFavorite => Boolean(favorite))
}

export async function addMarketFavorite(symbol: string): Promise<MarketFavorite> {
  const response = await client.put<{ favorite?: BackendMarketFavoriteRecord }>(
    favoriteUrl(symbol),
  )
  const favorite = mapMarketFavorite(response.data.favorite || {})
  if (!favorite) throw new Error('invalid market favorite response')
  return favorite
}

export async function removeMarketFavorite(symbol: string): Promise<void> {
  await client.delete(favoriteUrl(symbol))
}

function favoriteUrl(symbol: string): string {
  return requestUrl(`/user/market-favorites/${encodeURIComponent(normalizeSymbol(symbol))}`)
}
