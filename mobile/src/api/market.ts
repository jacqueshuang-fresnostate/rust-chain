import { client, publicApiRequestConfig, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import {
  DEFAULT_MARKET_KLINE_INTERVAL,
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
    const response = await client.get<{ markets?: BackendMarket[] }>(url, publicApiRequestConfig())
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
  const response = await client.get<{ markets?: BackendMarket[] }>(
    requestUrl('/markets'),
    publicApiRequestConfig(),
  )
  const markets = Array.isArray(response.data.markets) ? response.data.markets : []
  const results = await Promise.allSettled(
    markets.map((market) => client.get<BackendTicker>(
      requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(market.symbol))}/ticker`),
      publicApiRequestConfig(),
    )),
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
  interval: string = DEFAULT_MARKET_KLINE_INTERVAL,
  limit = DEFAULT_MARKET_KLINE_LIMIT,
): Promise<KlinePoint[]> {
  return fetchKlinePage(symbol, interval, Date.now(), limit)
}

/** Fetch real bars strictly before a Unix-millisecond cursor, across sparse gaps. */
export async function fetchOlderKlines(
  symbol: string,
  interval: string,
  before: number,
  limit = 100,
): Promise<KlinePoint[]> {
  if (!Number.isSafeInteger(before) || before <= 1) return []
  return fetchKlinePage(symbol, interval, before - 1, limit)
}

async function fetchKlinePage(symbol: string, interval: string, end: number, limit: number): Promise<KlinePoint[]> {
  // The backend returns the latest <=100 bars ending at an inclusive millisecond
  // bound. A calculated start would hide available history across trading gaps.
  limit = Number.isFinite(limit) ? Math.min(100, Math.max(1, Math.floor(limit))) : 100
  const response = await client.get<BackendKline[] | { klines?: BackendKline[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/klines`),
    publicApiRequestConfig({ params: { interval, end, limit } }),
  )
  const rawRows = Array.isArray(response.data) ? response.data : response.data.klines || []

  return mapMarketKlines(rawRows, limit).filter((point) => point.time <= end)
}

export async function fetchOrderBook(symbol: string): Promise<{ bids: OrderBookLevel[]; asks: OrderBookLevel[] }> {
  const response = await client.get<{ bids?: BackendDepthLevel[]; asks?: BackendDepthLevel[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/depth`),
    publicApiRequestConfig(),
  )
  return mapMarketDepthSnapshot(response.data)
}

export async function fetchRecentTrades(symbol: string, limit = 16): Promise<TradePrint[]> {
  const response = await client.get<{ trades?: BackendTrade[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/trades`),
    publicApiRequestConfig({ params: { limit } }),
  )
  const rows = Array.isArray(response.data.trades) ? response.data.trades : []
  return mapMarketTrades(rows, limit)
}
