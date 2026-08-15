import { normalizeSymbol } from './format.ts'
import type { MarketTicker } from './types.ts'

export type HomeMarketBriefTone = 'positive' | 'neutral' | 'negative'

export interface HomeMarketBrief {
  total: number
  rising: number
  falling: number
  unchanged: number
  advancingPercent: number
  tone: HomeMarketBriefTone
  focusTicker: MarketTicker
  topMover: MarketTicker
}

/**
 * 从真实 ticker 快照生成首页市场简报。
 *
 * 同一交易对只计入一次，优先保留 observedAt 更新的一条，避免重复市场
 * 放大上涨/下跌数量。无效价格或非有限涨跌幅不会进入市场广度样本。
 */
export function buildHomeMarketBrief(tickers: readonly MarketTicker[]): HomeMarketBrief | null {
  const validTickers = uniqueValidTickers(tickers)
  if (!validTickers.length) return null

  const rising = validTickers.filter((ticker) => ticker.changePercent > 0).length
  const falling = validTickers.filter((ticker) => ticker.changePercent < 0).length
  const unchanged = validTickers.length - rising - falling
  const advancingPercent = Math.round((rising / validTickers.length) * 100)
  const directionalTotal = rising + falling
  const directionalAdvancingPercent = directionalTotal > 0 ? (rising / directionalTotal) * 100 : 50
  const tone = directionalAdvancingPercent > 55
    ? 'positive'
    : directionalAdvancingPercent < 45
      ? 'negative'
      : 'neutral'

  const focusTicker = validTickers.find((ticker) => normalizeSymbol(ticker.symbol) === 'BTCUSDT')
    ?? [...validTickers].sort((left, right) => right.volume - left.volume)[0]!
  const topMover = [...validTickers].sort((left, right) => {
    if (right.changePercent !== left.changePercent) return right.changePercent - left.changePercent
    return right.volume - left.volume
  })[0]!

  return {
    total: validTickers.length,
    rising,
    falling,
    unchanged,
    advancingPercent,
    tone,
    focusTicker,
    topMover,
  }
}

function uniqueValidTickers(tickers: readonly MarketTicker[]): MarketTicker[] {
  const bySymbol = new Map<string, MarketTicker>()
  for (const ticker of tickers) {
    const symbol = normalizeSymbol(ticker.symbol)
    if (
      !symbol
      || !Number.isFinite(ticker.lastPrice)
      || ticker.lastPrice <= 0
      || !Number.isFinite(ticker.changePercent)
    ) {
      continue
    }

    const current = bySymbol.get(symbol)
    if (!current || observedAt(ticker) >= observedAt(current)) bySymbol.set(symbol, ticker)
  }
  return [...bySymbol.values()]
}

function observedAt(ticker: MarketTicker): number {
  const value = Number(ticker.observedAt)
  if (!Number.isFinite(value) || value <= 0) return 0
  return value < 1_000_000_000_000 ? value * 1000 : value
}
