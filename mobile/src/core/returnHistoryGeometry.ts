import type { ReturnHistory } from './returnHistory.ts'
import {
  decimalCompare,
  decimalDivide,
  decimalSign,
  decimalSubtract,
  decimalUnitRatioToNumber,
  normalizeDecimalText,
  type DecimalText,
} from './decimal.ts'

export type ReturnHistoryTone = 'positive' | 'negative' | 'neutral'

export interface ReturnHistoryGeometryPoint {
  x: number
  y: number
  value: DecimalText
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
  const zero = normalizeDecimalText('0')
  const values = [zero, ...cumulativeValues as DecimalText[]]

  let minimum = zero
  let maximum = zero
  for (const value of values) {
    if (decimalCompare(value, minimum) < 0) minimum = value
    if (decimalCompare(value, maximum) > 0) maximum = value
  }
  const range = decimalSubtract(maximum, minimum)
  const drawableHeight = CHART_HEIGHT - CHART_PADDING_Y * 2
  const denominator = values.length - 1
  const points = values.map((value, index) => ({
    x: denominator ? (index / denominator) * CHART_WIDTH : 0,
    y: decimalSign(range) === 0
      ? ZERO_LINE_Y
      : CHART_PADDING_Y + decimalUnitRatioToNumber(
        decimalDivide(decimalSubtract(maximum, value), range, 12),
      ) * drawableHeight,
    value,
  }))
  const latest = points.at(-1)
  if (!latest) return null
  const latestSign = decimalSign(latest.value)
  const tone = latestSign > 0 ? 'positive' : latestSign < 0 ? 'negative' : 'neutral'

  return {
    path: points
      .map((point, index) => `${index ? 'L' : 'M'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`)
      .join(' '),
    latest,
    points,
    tone,
  }
}
