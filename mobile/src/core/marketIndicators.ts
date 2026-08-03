import type { KlinePoint } from './types'

export const MARKET_MA_PERIODS = [5, 10, 20] as const

export interface MarketIndicatorPoint {
  time: number
  value: number
}

export interface MarketMovingAverages {
  ma5: MarketIndicatorPoint[]
  ma10: MarketIndicatorPoint[]
  ma20: MarketIndicatorPoint[]
}

export interface LatestMarketMovingAverages {
  ma5: number | null
  ma10: number | null
  ma20: number | null
}

type ClosingPoint = Pick<KlinePoint, 'time' | 'close'>

export function calculateSimpleMovingAverage(
  points: readonly ClosingPoint[],
  period: number,
): MarketIndicatorPoint[] {
  if (!Number.isInteger(period) || period <= 0) return []

  const rows: MarketIndicatorPoint[] = []
  const window: number[] = []
  let sum = 0

  for (const point of points) {
    if (!Number.isFinite(point.time) || !Number.isFinite(point.close) || point.close <= 0) {
      window.length = 0
      sum = 0
      continue
    }

    window.push(point.close)
    sum += point.close
    if (window.length > period) sum -= window.shift() ?? 0
    if (window.length === period) rows.push({ time: point.time, value: sum / period })
  }

  return rows
}

export function calculateMarketMovingAverages(
  points: readonly ClosingPoint[],
): MarketMovingAverages {
  return {
    ma5: calculateSimpleMovingAverage(points, MARKET_MA_PERIODS[0]),
    ma10: calculateSimpleMovingAverage(points, MARKET_MA_PERIODS[1]),
    ma20: calculateSimpleMovingAverage(points, MARKET_MA_PERIODS[2]),
  }
}

export function latestMarketMovingAverages(
  averages: MarketMovingAverages,
): LatestMarketMovingAverages {
  return {
    ma5: averages.ma5.at(-1)?.value ?? null,
    ma10: averages.ma10.at(-1)?.value ?? null,
    ma20: averages.ma20.at(-1)?.value ?? null,
  }
}
