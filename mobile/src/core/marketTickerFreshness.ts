import type { MarketTicker } from './types.ts'

export interface LiveMarketTickerUpdate {
  symbol: string
  lastPrice: number
  highPrice?: number
  lowPrice?: number
  volume?: number
  changePercent?: number
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

  const changePercent = finiteOptional(update.changePercent)
  const percentDenominator = changePercent === null ? 0 : 1 + changePercent / 100
  return {
    ...current,
    lastPrice: update.lastPrice,
    openPrice: changePercent !== null && percentDenominator > 0
      ? update.lastPrice / percentDenominator
      : current.openPrice,
    highPrice: positiveOptional(update.highPrice) ?? current.highPrice,
    lowPrice: positiveOptional(update.lowPrice) ?? current.lowPrice,
    volume: nonNegativeOptional(update.volume) ?? current.volume,
    changePercent: changePercent ?? current.changePercent,
    ...((updateObservedAt || currentObservedAt) > 0
      ? { observedAt: updateObservedAt || currentObservedAt }
      : {}),
  }
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

    return withLatestTickerSnapshot(snapshot, existing, existingObservedAt)
  })
}

/**
 * REST 快照晚于请求发起时间返回时，只吸收其中的市场元数据；价格、24 小时
 * 高低价、成交量和涨跌幅必须作为同一个带时间戳的行情快照整体保留，避免把
 * 新 WebSocket 价格与旧 REST 涨跌口径拼成一条内部不一致的数据。
 */
function withLatestTickerSnapshot(
  incoming: MarketTicker,
  current: MarketTicker,
  observedAt: number,
): MarketTicker {
  return {
    ...incoming,
    lastPrice: current.lastPrice,
    openPrice: current.openPrice,
    highPrice: current.highPrice,
    lowPrice: current.lowPrice,
    volume: current.volume,
    changePercent: current.changePercent,
    ...(observedAt > 0 ? { observedAt } : {}),
  }
}

function normalizeObservedAt(value: number | undefined): number {
  if (!Number.isFinite(value) || Number(value) <= 0) return 0
  const timestamp = Number(value)
  return timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function finiteOptional(value: number | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function positiveOptional(value: number | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null
}

function nonNegativeOptional(value: number | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function normalizeSymbol(value: string): string {
  return value.replace(/[-_/\s]/g, '').toUpperCase()
}
