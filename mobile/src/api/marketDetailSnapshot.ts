import type { KlinePoint, OrderBookLevel, TradePrint } from '../core/types.ts'
import type { MarketDetailStreamContext, MarketDetailStreamSession } from './marketDetailStream.ts'

type DepthSnapshot = { bids: OrderBookLevel[]; asks: OrderBookLevel[] }

interface MarketDetailSnapshotOptions {
  session: MarketDetailStreamSession
  context: MarketDetailStreamContext
  /** Symbol/load ownership, deliberately independent of the selected interval. */
  isCurrent(): boolean
  hasLiveDepth(): boolean
  loadKlines(): Promise<KlinePoint[]>
  loadDepth(): Promise<DepthSnapshot>
  loadTrades(): Promise<TradePrint[]>
  onKlines(points: KlinePoint[], failed: boolean): void
  /** Null preserves a live book, including an authoritative empty book. */
  onDepth(snapshot: DepthSnapshot | null, failed: boolean): void
  onTrades(trades: TradePrint[], failed: boolean): void
}

async function settle<T>(load: () => Promise<T>): Promise<{ value: T; failed: false } | { failed: true }> {
  try {
    return { value: await load(), failed: false }
  } catch {
    return { failed: true }
  }
}

/** Start together, commit independently. A slow channel never holds ready data. */
export async function loadMarketDetailSnapshot(options: MarketDetailSnapshotOptions): Promise<void> {
  const { session, context } = options
  const request = session.beginKlineRequest(context)
  await Promise.all([
    settle(options.loadKlines).then((result) => {
      if (!options.isCurrent() || !request || !session.isCurrentKlineRequest(request)) return
      const points = session.resolveKlineRequest(request, result.failed ? [] : result.value)
      if (points) options.onKlines(points, result.failed && points.length === 0)
    }),
    settle(options.loadDepth).then((result) => {
      if (!options.isCurrent()) return
      const live = options.hasLiveDepth()
      options.onDepth(live ? null : result.failed ? { bids: [], asks: [] } : result.value, !live && result.failed)
    }),
    settle(options.loadTrades).then((result) => {
      if (!options.isCurrent()) return
      options.onTrades(result.failed ? [] : result.value, result.failed)
    }),
  ])
}
