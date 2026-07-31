import type { OrderBookLevel, TradePrint } from '../core/types.ts'

export type MarketSocketChannel = 'ticker' | 'depth' | 'trade'

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
  | { type: 'pong' }

export function normalizeMarketSocketSymbol(symbol: string): string {
  return symbol.replace(/[-_/\s]/g, '').toUpperCase()
}

export function marketSubscriptionFrame(channel: MarketSocketChannel, symbol: string): string {
  return JSON.stringify({
    op: 'subscribe',
    channel,
    symbol: normalizeMarketSocketSymbol(symbol),
  })
}

export function tickerSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('ticker', symbol)
}

export function depthSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('depth', symbol)
}

export function tradeSubscriptionFrame(symbol: string): string {
  return marketSubscriptionFrame('trade', symbol)
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

function normalizeMarketTimestamp(value: unknown): number | null {
  if (
    (typeof value !== 'number' && typeof value !== 'string')
    || (typeof value === 'string' && !value.trim())
  ) {
    return null
  }
  const numberValue = Number(value)
  if (!Number.isFinite(numberValue) || numberValue <= 0) return null
  return numberValue < 1_000_000_000_000 ? numberValue * 1_000 : numberValue
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
