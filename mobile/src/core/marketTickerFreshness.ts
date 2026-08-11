import type { MarketTicker } from './types.ts'

export interface LiveMarketTickerUpdate {
  symbol: string
  lastPrice: number
  observedAt?: number
}

export function applyLiveMarketTickerUpdate(
  current: MarketTicker,
  update: LiveMarketTickerUpdate,
  receivedAt = Date.now(),
): MarketTicker {
  if (!Number.isFinite(update.lastPrice) || update.lastPrice <= 0) return current

  const currentObservedAt = normalizeObservedAt(current.observedAt)
  const updateObservedAt = normalizeObservedAt(update.observedAt) || normalizeObservedAt(receivedAt)
  if (currentObservedAt > 0 && updateObservedAt > 0 && updateObservedAt < currentObservedAt) {
    return current
  }

  return withLatestPrice(current, update.lastPrice, updateObservedAt || currentObservedAt)
}

export function mergeMarketTickerSnapshots(
  current: readonly MarketTicker[],
  incoming: readonly MarketTicker[],
): MarketTicker[] {
  const currentBySymbol = new Map(current.map((ticker) => [normalizeSymbol(ticker.symbol), ticker]))
  return incoming.map((snapshot) => {
    const existing = currentBySymbol.get(normalizeSymbol(snapshot.symbol))
    if (!existing) return snapshot

    const existingObservedAt = normalizeObservedAt(existing.observedAt)
    const incomingObservedAt = normalizeObservedAt(snapshot.observedAt)
    if (existingObservedAt <= 0 || incomingObservedAt <= 0 || existingObservedAt <= incomingObservedAt) {
      return snapshot
    }

    return withLatestPrice(snapshot, existing.lastPrice, existingObservedAt)
  })
}

function withLatestPrice(ticker: MarketTicker, lastPrice: number, observedAt: number): MarketTicker {
  return {
    ...ticker,
    lastPrice,
    changePercent: ticker.openPrice > 0
      ? ((lastPrice - ticker.openPrice) / ticker.openPrice) * 100
      : ticker.changePercent,
    ...(observedAt > 0 ? { observedAt } : {}),
  }
}

function normalizeObservedAt(value: number | undefined): number {
  if (!Number.isFinite(value) || Number(value) <= 0) return 0
  const timestamp = Number(value)
  return timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function normalizeSymbol(value: string): string {
  return value.replace(/[-_/\s]/g, '').toUpperCase()
}
