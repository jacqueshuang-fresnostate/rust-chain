import type { KlinePoint } from './types'

export interface NormalizedMarketChartPoint extends Omit<KlinePoint, 'time'> {
  time: number
}

/** 仅推导图表显示精度，不作为交易 tick 或金额校验规则，不改写 OHLC。 */
export function resolveMarketChartPriceFormat(
  points: readonly Pick<KlinePoint, 'open' | 'high' | 'low' | 'close'>[],
) {
  let precision = 2
  for (const point of points) {
    for (const value of [point.open, point.high, point.low, point.close]) {
      if (!Number.isFinite(value) || value <= 0) continue
      const [coefficient = '', exponent = '0'] = String(value).toLowerCase().split('e')
      const fraction = coefficient.split('.')[1]?.length ?? 0
      precision = Math.max(precision, Math.min(18, fraction - Number(exponent)))
    }
  }
  return { type: 'price' as const, precision, minMove: 10 ** -precision, base: 10 ** precision }
}

export function normalizeMarketChartTimestamp(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 0
  const milliseconds = value < 1_000_000_000_000 ? value * 1000 : value
  return Math.floor(milliseconds / 1000)
}

export function normalizeMarketChartPoints(points: KlinePoint[]): NormalizedMarketChartPoint[] {
  const unique = new Map<number, NormalizedMarketChartPoint>()

  for (const point of points) {
    const time = normalizeMarketChartTimestamp(point.time)
    if (
      time > 0
      && Number.isFinite(point.open)
      && Number.isFinite(point.high)
      && Number.isFinite(point.low)
      && Number.isFinite(point.close)
      && Number.isFinite(point.volume)
      && point.open > 0
      && point.high > 0
      && point.low > 0
      && point.close > 0
      && point.volume >= 0
    ) {
      unique.set(time, { ...point, time })
    }
  }

  return [...unique.values()].sort((left, right) => left.time - right.time)
}
