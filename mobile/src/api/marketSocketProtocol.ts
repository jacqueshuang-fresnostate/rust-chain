import type { KlinePoint, OrderBookLevel, TradePrint } from '../core/types.ts'

export const DEFAULT_MARKET_KLINE_LIMIT = 160
export const MARKET_KLINE_INTERVALS = ['1m', '5m', '15m', '1h', '1d'] as const

export type MarketKlineInterval = (typeof MARKET_KLINE_INTERVALS)[number]

export type MarketSocketChannel = 'ticker' | 'depth' | 'trade' | 'kline'

export type MarketSocketFrame =
  | { type: 'subscribed'; channel: string }
  | { type: 'ticker'; symbol: string; lastPrice: number; observedAt?: number }
  | {
      type: 'depth'
      symbol: string
      bids: OrderBookLevel[]
      asks: OrderBookLevel[]
      observedAt?: number
    }
  | { type: 'trade'; symbol: string; trade: TradePrint }
  | {
      type: 'kline'
      symbol: string
      interval: string
      point: KlinePoint
      observedAt: number
    }
  | { type: 'pong' }

export function normalizeMarketSocketSymbol(symbol: string): string {
  return symbol.replace(/[-_/\s]/g, '').toUpperCase()
}

export function normalizeMarketKlineInterval(interval: string): MarketKlineInterval | '' {
  const normalized = interval.trim().toLowerCase()
  return MARKET_KLINE_INTERVALS.includes(normalized as MarketKlineInterval)
    ? normalized as MarketKlineInterval
    : ''
}

export function marketSubscriptionFrame(
  channel: MarketSocketChannel,
  symbol: string,
  interval?: string,
): string {
  return marketSubscriptionCommandFrame('subscribe', channel, symbol, interval)
}

export function marketUnsubscriptionFrame(
  channel: MarketSocketChannel,
  symbol: string,
  interval?: string,
): string {
  return marketSubscriptionCommandFrame('unsubscribe', channel, symbol, interval)
}

function marketSubscriptionCommandFrame(
  operation: 'subscribe' | 'unsubscribe',
  channel: MarketSocketChannel,
  symbol: string,
  interval?: string,
): string {
  const payload: Record<string, string> = {
    op: operation,
    channel,
    symbol: normalizeMarketSocketSymbol(symbol),
  }
  if (channel === 'kline') {
    const normalizedInterval = normalizeMarketKlineInterval(interval ?? '')
    if (!normalizedInterval) throw new TypeError('A supported kline interval is required')
    payload.interval = normalizedInterval
  }
  return JSON.stringify(payload)
}

export function tickerSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('ticker', symbol)
}

export function tickerUnsubscriptionFrame(symbol: string): string {
  return marketUnsubscriptionFrame('ticker', symbol)
}

export function depthSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('depth', symbol)
}

export function tradeSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('trade', symbol)
}

export function klineSubscriptionFrame(symbol: string, interval: string): string {
  return marketSubscriptionFrame('kline', symbol, interval)
}

export function mapMarketKline(payload: unknown): KlinePoint | null {
  if (!isRecord(payload)) return null

  const time = normalizeMarketTimestamp(payload.open_time ?? payload.time ?? payload.timestamp)
  const open = positiveNumber(payload.open)
  const high = positiveNumber(payload.high)
  const low = positiveNumber(payload.low)
  const close = positiveNumber(payload.close)
  const volume = nonNegativeNumber(payload.volume)
  if (
    time === null
    || open === null
    || high === null
    || low === null
    || close === null
    || volume === null
  ) {
    return null
  }

  return normalizeKlinePoint({ time, open, high, low, close, volume })
}

export function mapMarketKlines(
  rows: unknown,
  limit = DEFAULT_MARKET_KLINE_LIMIT,
): KlinePoint[] {
  if (!Array.isArray(rows)) return []
  const points = rows
    .map(mapMarketKline)
    .filter((point): point is KlinePoint => point !== null)
  return mergeMarketKlines(points, [], limit)
}

export function mergeMarketKlines(
  primary: readonly KlinePoint[],
  secondary: readonly KlinePoint[],
  limit = DEFAULT_MARKET_KLINE_LIMIT,
): KlinePoint[] {
  const normalizedLimit = normalizeLimit(limit, DEFAULT_MARKET_KLINE_LIMIT)
  if (normalizedLimit === 0) return []

  const unique = new Map<number, KlinePoint>()
  for (const point of [...secondary, ...primary]) {
    const normalized = normalizeKlinePoint(point)
    if (normalized) unique.set(normalized.time, normalized)
  }
  return [...unique.values()]
    .sort((left, right) => left.time - right.time)
    .slice(-normalizedLimit)
}

export function mapMarketDepthSnapshot(
  payload: { bids?: unknown; asks?: unknown },
  limit = 12,
): { bids: OrderBookLevel[]; asks: OrderBookLevel[] } {
  const normalizedLimit = normalizeLimit(limit, 12)
  return {
    bids: mapDepthLevels(payload.bids)
      .sort((left, right) => right.price - left.price)
      .slice(0, normalizedLimit),
    asks: mapDepthLevels(payload.asks)
      .sort((left, right) => left.price - right.price)
      .slice(0, normalizedLimit),
  }
}

export function mapMarketTrade(payload: unknown, fallbackId = ''): TradePrint | null {
  if (!isRecord(payload)) return null

  const rawId = payload.trade_id ?? payload.id ?? fallbackId
  const id = typeof rawId === 'string' || typeof rawId === 'number'
    ? String(rawId).trim()
    : ''
  const side = String(payload.side ?? payload.direction ?? '').trim().toLowerCase()
  const price = positiveNumber(payload.price)
  const quantity = positiveNumber(payload.quantity ?? payload.amount)
  const time = normalizeMarketTimestamp(payload.traded_at ?? payload.time)

  if (!id || (side !== 'buy' && side !== 'sell') || price === null || quantity === null || time === null) {
    return null
  }

  return {
    id,
    side,
    price,
    quantity,
    time,
  }
}

export function mapMarketTrades(rows: unknown, limit = 16): TradePrint[] {
  if (!Array.isArray(rows)) return []

  const sorted = rows
    .map((row, index) => mapMarketTrade(row, `rest-${index}`))
    .filter((trade): trade is TradePrint => trade !== null)
    .sort((left, right) => right.time - left.time)
  return mergeMarketTradeHistory(sorted, [], limit)
}

export function mergeMarketTradeHistory(
  primary: readonly TradePrint[],
  secondary: readonly TradePrint[],
  limit = 16,
): TradePrint[] {
  const seen = new Set<string>()
  return [...primary, ...secondary]
    .filter((trade) => {
      if (!isValidTradePrint(trade) || seen.has(trade.id)) return false
      seen.add(trade.id)
      return true
    })
    .slice(0, normalizeLimit(limit, 16))
}

export function mergeMarketTrades(
  current: readonly TradePrint[],
  incoming: TradePrint,
  limit = 16,
): TradePrint[] {
  const normalizedCurrent = mergeMarketTradeHistory(current, [], limit)
  if (
    !isValidTradePrint(incoming)
    || normalizedCurrent.some((trade) => trade.id === incoming.id)
  ) {
    return normalizedCurrent
  }
  return [incoming, ...normalizedCurrent].slice(0, normalizeLimit(limit, 16))
}

export function parseMarketSocketFrame(data: unknown): MarketSocketFrame | null {
  if (typeof data !== 'string') return null
  if (data.trim().toLowerCase() === 'pong') return { type: 'pong' }

  try {
    const payload: unknown = JSON.parse(data)
    if (!isRecord(payload)) return null
    if (payload.type === 'pong') return { type: 'pong' }
    if (payload.type === 'subscribed' && typeof payload.channel === 'string') {
      return { type: 'subscribed', channel: payload.channel }
    }
    if (typeof payload.symbol !== 'string' || !normalizeMarketSocketSymbol(payload.symbol)) {
      return null
    }

    const symbol = payload.symbol
    if (Array.isArray(payload.bids) && Array.isArray(payload.asks)) {
      if (!hasOnlyValidDepthLevels(payload.bids) || !hasOnlyValidDepthLevels(payload.asks)) {
        return null
      }
      const snapshot = mapMarketDepthSnapshot(payload)
      const observedAt = optionalTimestamp(payload.observed_at)
      if (observedAt === null) return null
      return {
        type: 'depth',
        symbol,
        bids: snapshot.bids,
        asks: snapshot.asks,
        ...(observedAt === undefined ? {} : { observedAt }),
      }
    }

    if (
      payload.type === 'kline'
      || payload.interval !== undefined
      || payload.open_time !== undefined
    ) {
      if (!isDirectMarketKlinePayload(payload)) return null
      const interval = typeof payload.interval === 'string'
        ? normalizeMarketKlineInterval(payload.interval)
        : ''
      const point = mapMarketKline(payload)
      const observedAt = normalizeMarketTimestamp(payload.observed_at)
      if (!interval || !point || observedAt === null) return null
      return {
        type: 'kline',
        symbol,
        interval,
        point,
        observedAt,
      }
    }

    if (payload.trade_id !== undefined || payload.type === 'trade') {
      const trade = mapMarketTrade(payload)
      if (!trade) return null
      return { type: 'trade', symbol, trade }
    }

    const lastPrice = positiveNumber(payload.last_price)
    if (lastPrice === null) return null
    const observedAt = optionalTimestamp(payload.observed_at)
    if (observedAt === null) return null
    return {
      type: 'ticker',
      symbol,
      lastPrice,
      ...(observedAt === undefined ? {} : { observedAt }),
    }
  } catch {
    return null
  }
}

function mapDepthLevels(rows: unknown): OrderBookLevel[] {
  if (!Array.isArray(rows)) return []
  return rows
    .map((row) => {
      if (!isRecord(row)) return null
      const price = positiveNumber(row.price)
      const quantity = positiveNumber(row.quantity ?? row.amount)
      return price === null || quantity === null ? null : { price, quantity }
    })
    .filter((row): row is OrderBookLevel => row !== null)
}

function hasOnlyValidDepthLevels(rows: unknown[]): boolean {
  return rows.every((row) => {
    if (!isRecord(row)) return false
    return positiveNumber(row.price) !== null
      && positiveNumber(row.quantity ?? row.amount) !== null
  })
}

function isDirectMarketKlinePayload(payload: Record<string, unknown>): boolean {
  return typeof payload.interval === 'string'
    && Boolean(normalizeMarketKlineInterval(payload.interval))
    && isUnixMillisecondTimestamp(payload.open_time)
    && isPositiveDecimalString(payload.open)
    && isPositiveDecimalString(payload.high)
    && isPositiveDecimalString(payload.low)
    && isPositiveDecimalString(payload.close)
    && isNonNegativeDecimalString(payload.volume)
    && isUnixMillisecondTimestamp(payload.observed_at)
    && typeof payload.provider === 'string'
    && Boolean(payload.provider.trim())
}

function normalizeKlinePoint(point: KlinePoint): KlinePoint | null {
  const time = normalizeMarketTimestamp(point.time)
  if (
    time === null
    || !Number.isFinite(point.open)
    || !Number.isFinite(point.high)
    || !Number.isFinite(point.low)
    || !Number.isFinite(point.close)
    || !Number.isFinite(point.volume)
    || point.open <= 0
    || point.high <= 0
    || point.low <= 0
    || point.close <= 0
    || point.volume < 0
    || point.high < Math.max(point.open, point.low, point.close)
    || point.low > Math.min(point.open, point.high, point.close)
  ) {
    return null
  }
  return { ...point, time }
}

function positiveNumber(value: unknown): number | null {
  if (
    (typeof value !== 'number' && typeof value !== 'string')
    || (typeof value === 'string' && !value.trim())
  ) {
    return null
  }
  const numberValue = Number(value)
  return Number.isFinite(numberValue) && numberValue > 0 ? numberValue : null
}

function nonNegativeNumber(value: unknown): number | null {
  if (
    (typeof value !== 'number' && typeof value !== 'string')
    || (typeof value === 'string' && !value.trim())
  ) {
    return null
  }
  const numberValue = Number(value)
  return Number.isFinite(numberValue) && numberValue >= 0 ? numberValue : null
}

function isPositiveDecimalString(value: unknown): boolean {
  return isDecimalString(value) && Number(value) > 0
}

function isNonNegativeDecimalString(value: unknown): boolean {
  return isDecimalString(value) && Number(value) >= 0
}

function isDecimalString(value: unknown): value is string {
  if (typeof value !== 'string' || value !== value.trim() || !value) return false
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value)) return false
  return Number.isFinite(Number(value))
}

function isUnixMillisecondTimestamp(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 1_000_000_000_000
}

function normalizeMarketTimestamp(value: unknown): number | null {
  if (
    (typeof value !== 'number' && typeof value !== 'string')
    || (typeof value === 'string' && !value.trim())
  ) {
    return null
  }
  const numberValue = Number(value)
  if (!Number.isSafeInteger(numberValue) || numberValue <= 0) return null
  const milliseconds = numberValue < 1_000_000_000_000 ? numberValue * 1_000 : numberValue
  return Number.isSafeInteger(milliseconds) ? milliseconds : null
}

function optionalTimestamp(value: unknown): number | null | undefined {
  return value === undefined ? undefined : normalizeMarketTimestamp(value)
}

function isValidTradePrint(trade: TradePrint): boolean {
  return typeof trade.id === 'string'
    && Boolean(trade.id.trim())
    && (trade.side === 'buy' || trade.side === 'sell')
    && Number.isFinite(trade.price)
    && trade.price > 0
    && Number.isFinite(trade.quantity)
    && trade.quantity > 0
    && Number.isFinite(trade.time)
    && trade.time > 0
}

function normalizeLimit(value: number, fallback: number): number {
  return Number.isFinite(value) && value >= 0 ? Math.floor(value) : fallback
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
