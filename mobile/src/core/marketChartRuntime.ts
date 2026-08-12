import type { NormalizedMarketChartPoint } from './marketChart'

export type MarketChartDataUpdate = 'none' | 'update-last' | 'append' | 'replace'

export interface MarketChartLogicalRange {
  from: number
  to: number
}

export interface MarketChartLogicalViewport {
  rangeWidth: number
  anchorTimestamp: number
  anchorOffset: number
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

  const anchorIndex = nearestTimestampIndex(rows, viewport.anchorTimestamp)
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

function nearestTimestampIndex(
  rows: readonly { time: number }[],
  timestamp: number,
): number | null {
  let nearestIndex: number | null = null
  let nearestDistance = Number.POSITIVE_INFINITY
  for (const [index, row] of rows.entries()) {
    if (!Number.isFinite(row.time) || row.time <= 0) continue
    const distance = Math.abs(row.time - timestamp)
    if (distance < nearestDistance) {
      nearestIndex = index
      nearestDistance = distance
    }
    if (distance === 0) break
  }
  return nearestIndex
}
