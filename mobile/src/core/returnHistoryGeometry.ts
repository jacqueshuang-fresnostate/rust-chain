import type { ReturnHistory } from './returnHistory.ts'

export type ReturnHistoryTone = 'positive' | 'negative' | 'neutral'

export interface ReturnHistoryGeometryPoint {
  x: number
  y: number
  value: number
}

export interface ReturnHistoryGeometry {
  path: string
  latest: ReturnHistoryGeometryPoint
  points: ReturnHistoryGeometryPoint[]
  tone: ReturnHistoryTone
}

const CHART_WIDTH = 358
const CHART_HEIGHT = 153
const CHART_PADDING_Y = 12
const ZERO_LINE_Y = CHART_HEIGHT / 2

export function buildReturnHistoryGeometry(history: ReturnHistory): ReturnHistoryGeometry | null {
  if (history.status !== 'complete') return null
  const cumulativeValues = history.points.map((point) => point.cumulativeAmount)
  if (cumulativeValues.some((value) => value === null)) return null
  const values = [0, ...cumulativeValues as number[]]
  if (values.some((value) => !Number.isFinite(value))) return null

  const minimum = Math.min(0, ...values)
  const maximum = Math.max(0, ...values)
  const range = maximum - minimum
  const drawableHeight = CHART_HEIGHT - CHART_PADDING_Y * 2
  const denominator = values.length - 1
  const points = values.map((value, index) => ({
    x: denominator ? (index / denominator) * CHART_WIDTH : 0,
    y: range === 0
      ? ZERO_LINE_Y
      : CHART_PADDING_Y + ((maximum - value) / range) * drawableHeight,
    value,
  }))
  const latest = points.at(-1)
  if (!latest) return null
  const tone = latest.value > 0 ? 'positive' : latest.value < 0 ? 'negative' : 'neutral'

  return {
    path: points
      .map((point, index) => `${index ? 'L' : 'M'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`)
      .join(' '),
    latest,
    points,
    tone,
  }
}
