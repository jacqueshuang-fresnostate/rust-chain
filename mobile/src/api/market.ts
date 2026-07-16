import { client, requestUrl } from './client'
import { asNumber, normalizeSymbol } from '@/core/format'
import { mapMarketTicker, type BackendMarketRecord, type BackendTickerRecord } from '@/core/marketMapper'
import type { KlinePoint, MarketTicker, OrderBookLevel, TradePrint } from '@/core/types'

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
  side?: string
  direction?: string
  price?: number | string
  quantity?: number | string
  amount?: number | string
  traded_at?: number | string
  time?: number | string
}

export { mapMarketTicker }

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

export async function fetchKlines(symbol: string, interval = '15m', limit = 160): Promise<KlinePoint[]> {
  const end = Date.now()
  const start = end - intervalDuration(interval) * limit
  const response = await client.get<BackendKline[] | { klines?: BackendKline[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/klines`),
    { params: { interval, start, end, limit } },
  )
  const rawRows = Array.isArray(response.data) ? response.data : response.data.klines || []

  return rawRows
    .map((row) => ({
      time: normalizeTimestamp(row.open_time ?? row.time ?? row.timestamp),
      open: asNumber(row.open),
      high: asNumber(row.high),
      low: asNumber(row.low),
      close: asNumber(row.close),
      volume: asNumber(row.volume),
    }))
    .filter((row) => row.time > 0 && row.high > 0 && row.low > 0)
    .sort((left, right) => left.time - right.time)
}

export async function fetchOrderBook(symbol: string): Promise<{ bids: OrderBookLevel[]; asks: OrderBookLevel[] }> {
  const response = await client.get<{ bids?: BackendDepthLevel[]; asks?: BackendDepthLevel[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/depth`),
  )
  return {
    bids: mapDepth(response.data.bids).sort((left, right) => right.price - left.price),
    asks: mapDepth(response.data.asks).sort((left, right) => left.price - right.price),
  }
}

export async function fetchRecentTrades(symbol: string, limit = 16): Promise<TradePrint[]> {
  const response = await client.get<{ trades?: BackendTrade[] }>(
    requestUrl(`/markets/${encodeURIComponent(normalizeSymbol(symbol))}/trades`),
    { params: { limit } },
  )
  const rows = Array.isArray(response.data.trades) ? response.data.trades : []
  return rows.map((trade, index) => ({
    id: String(trade.id ?? `${trade.price}-${index}`),
    side: String(trade.side || trade.direction || '').toLowerCase() === 'sell' ? 'sell' : 'buy',
    price: asNumber(trade.price),
    quantity: asNumber(trade.quantity ?? trade.amount),
    time: normalizeTimestamp(trade.traded_at ?? trade.time) || Date.now(),
  }))
}

function mapDepth(rows: BackendDepthLevel[] | undefined): OrderBookLevel[] {
  return (rows || [])
    .map((row) => ({ price: asNumber(row.price), quantity: asNumber(row.quantity ?? row.amount) }))
    .filter((row) => row.price > 0 && row.quantity > 0)
    .slice(0, 12)
}

function intervalDuration(interval: string): number {
  const normalized = interval.toLowerCase()
  if (normalized.endsWith('h')) return asNumber(normalized.slice(0, -1), 1) * 60 * 60 * 1000
  if (normalized.endsWith('d')) return asNumber(normalized.slice(0, -1), 1) * 24 * 60 * 60 * 1000
  return asNumber(normalized.replace('m', ''), 15) * 60 * 1000
}

function normalizeTimestamp(value: unknown): number {
  const time = asNumber(value)
  return time > 0 && time < 1_000_000_000_000 ? time * 1000 : time
}
