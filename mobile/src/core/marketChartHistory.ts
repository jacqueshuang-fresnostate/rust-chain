import { normalizeMarketChartPoints, type NormalizedMarketChartPoint } from './marketChart.ts'
import type { KlinePoint } from './types'

export interface MarketChartHistorySnapshot {
  points: NormalizedMarketChartPoint[]
  status: 'idle' | 'loading' | 'error' | 'exhausted'
}

interface HistoryInput {
  symbol: string
  interval: string
  points: KlinePoint[]
  loading: boolean
}

/** The parent feed owns current/live rows; this session owns only user-requested history. */
export function createMarketChartHistorySession(options: {
  loadOlder: (symbol: string, interval: string, before: number) => Promise<KlinePoint[]>
  onChange?: (snapshot: MarketChartHistorySnapshot) => void
}) {
  let state: MarketChartHistorySnapshot = { points: [], status: 'idle' }
  let input: HistoryInput | null = null
  let generation = 0
  let browsing = false
  let awaitingPoints = false
  let disposed = false

  function publish(next: MarketChartHistorySnapshot): void {
    state = next
    options.onChange?.(state)
  }

  function sync(next: HistoryInput): void {
    if (disposed) return
    const normalized = { ...next, symbol: next.symbol.trim().toUpperCase() }
    const keyChanged = !input || input.symbol !== normalized.symbol || input.interval !== next.interval
    const reloading = Boolean(input && !input.loading && next.loading)
    const pointsChanged = input?.points !== next.points
    if (keyChanged || reloading) {
      generation += 1
      browsing = false
      awaitingPoints = keyChanged && !pointsChanged
      publish({ points: [], status: 'idle' })
    }
    input = normalized
    if (awaitingPoints && !pointsChanged) return
    awaitingPoints = false
    if (pointsChanged || keyChanged || reloading) {
      const points = normalizeMarketChartPoints(browsing ? [...state.points, ...next.points] : next.points)
      publish({ points, status: state.status })
    }
  }

  async function requestOlder(): Promise<void> {
    if (disposed || !input || input.loading || awaitingPoints || !input.symbol || !input.interval
      || state.status !== 'idle' || !state.points.length) return
    const owner = generation
    const before = state.points[0].time * 1000
    const { symbol, interval } = input
    browsing = true
    publish({ ...state, status: 'loading' })
    try {
      const response = await options.loadOlder(symbol, interval, before)
      if (disposed || owner !== generation) return
      const older = normalizeMarketChartPoints(response).filter((point) => point.time * 1000 < before)
      const knownTimes = new Set(state.points.map((point) => point.time))
      const progressed = older.some((point) => !knownTimes.has(point.time))
      // Current rows always win, including a live update received while REST was pending.
      const points = progressed ? normalizeMarketChartPoints([...older, ...state.points]) : state.points
      publish({ points, status: progressed ? 'idle' : 'exhausted' })
    } catch {
      if (!disposed && owner === generation) publish({ ...state, status: 'error' })
    }
  }

  return {
    sync,
    requestOlder,
    async retry(): Promise<void> {
      if (disposed || state.status !== 'error') return
      publish({ ...state, status: 'idle' })
      await requestOlder()
    },
    snapshot: () => state,
    dispose(): void { disposed = true; generation += 1 },
  }
}
