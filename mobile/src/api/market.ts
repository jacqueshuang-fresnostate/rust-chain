import { client, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import {
  DEFAULT_MARKET_KLINE_LIMIT,
  mapMarketDepthSnapshot,
  mapMarketKlines,
  mapMarketTrades,
} from './marketSocketProtocol'
import { asNumber, normalizeSymbol, splitSymbol } from '@/core/format'
import { mapMarketTicker, type BackendMarketRecord, type BackendTickerRecord } from '@/core/marketMapper'
import type { KlinePoint, MarketPair, MarketTicker, OrderBookLevel, TradePrint } from '@/core/types'

type BackendMarket = BackendMarketRecord
type BackendTicker = BackendTickerRecord

interface BackendKline {
  open_time?: number | string
  time?: number | string
  timestamp?: number | string
  open?: number | string
  high?: number | string
  low?: number | string
  close?: number | string
  volume?: number | string
}

interface BackendDepthLevel {
  price?: number | string
  amount?: number | string
  quantity?: number | string
}

interface BackendTrade {
  id?: string | number
  trade_id?: string | number
  side?: string
  direction?: string
  price?: number | string
  quantity?: number | string
  amount?: number | string
  traded_at?: number | string
  time?: number | string
}

export { mapMarketTicker }

export async function fetchMarketPairs(options: ReferenceRequestOptions = {}): Promise<MarketPair[]> {
  const url = requestUrl('/markets')
  // 只缓存公开交易对元数据；ticker、K 线、深度与成交始终走实时请求。
  return referenceRequestRegistry.request(createReferenceRequestKey(url, { projection: 'pairs' }), 2 * 60_000, async () => {
    const response = await client.get<{ markets?: BackendMarket[] }>(url)
    return (response.data.markets || [])
      .map((market) => {
        const pair = splitSymbol(market.symbol, market.base_asset, market.quote_asset)
        return {
          id: asNumber(market.id),
          symbol: `${pair.base}/${pair.quote}`,
          base: pair.base,
          quote: pair.quote,
        }
      })
      .filter((pair) => pair.id > 0 && Boolean(pair.base && pair.quote))
  }, options)
}

export async function fetchMarketTickers(): Promise<MarketTicker[]> {
  const response = await client.get<{ markets?: BackendMarket[] }>(requestUrl('/markets'))
  const markets = Array.isArray(response.data.markets) ? response.data.markets : []
  const results = await Promise.allSettled(
    markets.map((market) => client.get<BackendTicker>(requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(market.symbol))}/ticker`))),
  )

  return markets
    .map((market, index) => {
      const result = results[index]
      return result?.status === 'fulfilled' ? mapMarketTicker(market, result.value.data) : null
    })
    .filter((ticker): ticker is MarketTicker => Boolean(ticker && ticker.lastPrice > 0))
    .sort((left, right) => right.volume - left.volume)
}

export async function fetchKlines(
  symbol: string,
  interval = '15m',
  limit = DEFAULT_MARKET_KLINE_LIMIT,
): Promise<KlinePoint[]> {
  const end = Date.now()
  const start = end - intervalDuration(interval) * limit
  const response = await client.get<BackendKline[] | { klines?: BackendKline[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/klines`),
    { params: { interval, start, end, limit } },
  )
  const rawRows = Array.isArray(response.data) ? response.data : response.data.klines || []

  return mapMarketKlines(rawRows, limit)
}

export async function fetchOrderBook(symbol: string): Promise<{ bids: OrderBookLevel[]; asks: OrderBookLevel[] }> {
  const response = await client.get<{ bids?: BackendDepthLevel[]; asks?: BackendDepthLevel[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/depth`),
  )
  return mapMarketDepthSnapshot(response.data)
}

export async function fetchRecentTrades(symbol: string, limit = 16): Promise<TradePrint[]> {
  const response = await client.get<{ trades?: BackendTrade[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/trades`),
    { params: { limit } },
  )
  const rows = Array.isArray(response.data.trades) ? response.data.trades : []
  return mapMarketTrades(rows, limit)
}

function intervalDuration(interval: string): number {
  const normalized = interval.toLowerCase()
  if (normalized.endsWith('h')) return asNumber(normalized.slice(0, -1), 1) * 60 * 60 * 1000
  if (normalized.endsWith('d')) return asNumber(normalized.slice(0, -1), 1) * 24 * 60 * 60 * 1000
  return asNumber(normalized.replace('m', ''), 15) * 60 * 1000
}
