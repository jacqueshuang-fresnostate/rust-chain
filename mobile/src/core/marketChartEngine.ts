import type { NormalizedMarketChartPoint } from './marketChart'

export const MARKET_CHART_ENGINE_STORAGE_KEY = 'hippo_mobile_market_chart_engine'
export const DEFAULT_MARKET_CHART_ENGINE = 'klinecharts' as const

export type MarketChartEngine = 'klinecharts' | 'tradingview'
export type MarketChartDataUpdate = 'none' | 'update-last' | 'append' | 'replace'
export type MarketChartPeriodType = 'second' | 'minute' | 'hour' | 'day' | 'week' | 'month' | 'year'

export interface MarketChartPeriod {
  type: MarketChartPeriodType
  span: number
}

export interface MarketChartVisibleRange {
  to: number
  realTo: number
}

export interface MarketChartViewport {
  barSpace: number
  anchorTimestamp: number
  anchorOffset: number
}

export interface MarketChartLogicalRange {
  from: number
  to: number
}

export interface MarketChartLogicalViewport {
  rangeWidth: number
  anchorTimestamp: number
  anchorOffset: number
}

export interface MarketChartSymbolInfo {
  ticker: string
  pricePrecision: number
  volumePrecision: number
}

export interface MarketChartEngineStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export function normalizeMarketChartEngine(value: unknown): MarketChartEngine {
  return value === 'tradingview' ? 'tradingview' : DEFAULT_MARKET_CHART_ENGINE
}

export function loadMarketChartEngine(
  storage: MarketChartEngineStorage | null = browserMarketChartStorage(),
): MarketChartEngine {
  if (!storage) return DEFAULT_MARKET_CHART_ENGINE
  try {
    return normalizeMarketChartEngine(storage.getItem(MARKET_CHART_ENGINE_STORAGE_KEY))
  } catch {
    return DEFAULT_MARKET_CHART_ENGINE
  }
}

export function persistMarketChartEngine(
  engine: MarketChartEngine,
  storage: MarketChartEngineStorage | null = browserMarketChartStorage(),
): void {
  if (!storage) return
  try {
    storage.setItem(MARKET_CHART_ENGINE_STORAGE_KEY, engine)
  } catch {
    return
  }
}

export function marketChartPeriod(interval: string): MarketChartPeriod {
  const match = /^(\d+)(s|m|h|d|w|M|y)$/.exec(interval)
  const span = Number(match?.[1])
  const unit = match?.[2]
  if (!Number.isSafeInteger(span) || span <= 0 || !unit) return { type: 'minute', span: 15 }

  const types: Record<string, MarketChartPeriodType> = {
    s: 'second',
    m: 'minute',
    h: 'hour',
    d: 'day',
    w: 'week',
    M: 'month',
    y: 'year',
  }
  return { type: types[unit] ?? 'minute', span }
}

export function normalizeMarketChartTicker(symbol: string): string {
  const ticker = symbol.trim().toUpperCase().replace(/[_-]+/g, '/').replace(/\/{2,}/g, '/')
  return ticker || '--'
}

export function resolveMarketChartSymbolInfo(
  symbol: string,
  points: readonly Pick<NormalizedMarketChartPoint, 'close' | 'volume'>[],
): MarketChartSymbolInfo {
  const currentPrice = latestPositiveValue(points, 'close')
  const currentVolume = latestPositiveValue(points, 'volume')
  return {
    ticker: normalizeMarketChartTicker(symbol),
    pricePrecision: pricePrecisionFor(currentPrice),
    volumePrecision: volumePrecisionFor(currentVolume),
  }
}

export function classifyMarketChartDataUpdate(
  previous: readonly NormalizedMarketChartPoint[],
  next: readonly NormalizedMarketChartPoint[],
): MarketChartDataUpdate {
  if (previous.length === next.length && previous.every((point, index) => (
    sameMarketChartPoint(point, next[index])
  ))) {
    return 'none'
  }

  if (
    previous.length > 0
    && previous.length === next.length
    && previous.slice(0, -1).every((point, index) => sameMarketChartPoint(point, next[index]))
    && previous.at(-1)?.time === next.at(-1)?.time
  ) {
    return 'update-last'
  }

  if (
    previous.length > 0
    && next.length === previous.length + 1
    && previous.every((point, index) => sameMarketChartPoint(point, next[index]))
    && (next.at(-1)?.time ?? 0) > (previous.at(-1)?.time ?? 0)
  ) {
    return 'append'
  }

  return 'replace'
}

export function captureMarketChartViewport(
  rows: readonly { timestamp: number }[],
  range: MarketChartVisibleRange,
  barSpace: number,
): MarketChartViewport | null {
  if (
    !rows.length
    || !Number.isFinite(range.to)
    || !Number.isFinite(range.realTo)
    || !Number.isFinite(barSpace)
    || barSpace <= 0
  ) {
    return null
  }

  const anchorIndex = Math.min(
    rows.length - 1,
    Math.max(0, Math.ceil(range.to) - 1),
  )
  const anchorTimestamp = rows[anchorIndex]?.timestamp
  if (!Number.isFinite(anchorTimestamp) || anchorTimestamp <= 0) return null

  return {
    barSpace,
    anchorTimestamp,
    anchorOffset: range.realTo - anchorIndex,
  }
}

export function resolveMarketChartViewportRealTo(
  rows: readonly { timestamp: number }[],
  viewport: MarketChartViewport,
): number | null {
  if (
    !rows.length
    || !Number.isFinite(viewport.anchorTimestamp)
    || viewport.anchorTimestamp <= 0
    || !Number.isFinite(viewport.anchorOffset)
  ) {
    return null
  }

  const anchorIndex = nearestTimestampIndex(rows, 'timestamp', viewport.anchorTimestamp)
  if (anchorIndex === null) return null
  return anchorIndex + viewport.anchorOffset
}

export function captureMarketChartLogicalViewport(
  rows: readonly { time: number }[],
  range: MarketChartLogicalRange | null,
): MarketChartLogicalViewport | null {
  if (
    !rows.length
    || !range
    || !Number.isFinite(range.from)
    || !Number.isFinite(range.to)
    || range.to <= range.from
  ) {
    return null
  }

  const anchorIndex = Math.min(
    rows.length - 1,
    Math.max(0, Math.ceil(range.to) - 1),
  )
  const anchorTimestamp = rows[anchorIndex]?.time
  if (!Number.isFinite(anchorTimestamp) || anchorTimestamp <= 0) return null

  return {
    rangeWidth: range.to - range.from,
    anchorTimestamp,
    anchorOffset: range.to - anchorIndex,
  }
}

export function resolveMarketChartLogicalRange(
  rows: readonly { time: number }[],
  viewport: MarketChartLogicalViewport,
): MarketChartLogicalRange | null {
  if (
    !rows.length
    || !Number.isFinite(viewport.rangeWidth)
    || viewport.rangeWidth <= 0
    || !Number.isFinite(viewport.anchorTimestamp)
    || viewport.anchorTimestamp <= 0
    || !Number.isFinite(viewport.anchorOffset)
  ) {
    return null
  }

  const anchorIndex = nearestTimestampIndex(rows, 'time', viewport.anchorTimestamp)
  if (anchorIndex === null) return null
  const to = anchorIndex + viewport.anchorOffset
  return { from: to - viewport.rangeWidth, to }
}

function sameMarketChartPoint(
  left: NormalizedMarketChartPoint,
  right: NormalizedMarketChartPoint | undefined,
): boolean {
  return Boolean(
    right
    && left.time === right.time
    && left.open === right.open
    && left.high === right.high
    && left.low === right.low
    && left.close === right.close
    && left.volume === right.volume,
  )
}

function nearestTimestampIndex<T extends 'time' | 'timestamp'>(
  rows: readonly Record<T, number>[],
  key: T,
  timestamp: number,
): number | null {
  let nearestIndex: number | null = null
  let nearestDistance = Number.POSITIVE_INFINITY
  for (const [index, row] of rows.entries()) {
    const value = row[key]
    if (!Number.isFinite(value) || value <= 0) continue
    const distance = Math.abs(value - timestamp)
    if (distance < nearestDistance) {
      nearestIndex = index
      nearestDistance = distance
    }
    if (distance === 0) break
  }
  return nearestIndex
}

function latestPositiveValue(
  points: readonly Pick<NormalizedMarketChartPoint, 'close' | 'volume'>[],
  key: 'close' | 'volume',
): number {
  for (let index = points.length - 1; index >= 0; index -= 1) {
    const value = points[index]?.[key]
    if (Number.isFinite(value) && Number(value) > 0) return Number(value)
  }
  return 0
}

function pricePrecisionFor(price: number): number {
  if (price >= 100) return 2
  if (price >= 1) return 4
  if (price >= .1) return 5
  if (price >= .01) return 6
  if (price >= .001) return 7
  return price > 0 ? 8 : 2
}

function volumePrecisionFor(volume: number): number {
  if (volume >= 1_000) return 0
  if (volume >= 100) return 1
  if (volume >= 1) return 2
  if (volume >= .01) return 4
  return volume > 0 ? 6 : 2
}

function browserMarketChartStorage(): MarketChartEngineStorage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}
