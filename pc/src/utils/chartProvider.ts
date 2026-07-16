export const DEFAULT_CHART_PROVIDER = 'klinecharts' as const

export type PcChartProvider = typeof DEFAULT_CHART_PROVIDER | 'tradingview'

export function normalizeChartProvider(value: unknown): PcChartProvider {
  return String(value ?? '').trim().toLowerCase() === 'tradingview'
    ? 'tradingview'
    : DEFAULT_CHART_PROVIDER
}
