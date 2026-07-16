export type KlineModule = 'spot' | 'margin' | 'seconds' | 'market' | 'second' | 'swap'
export type MarketWsModule = 'spot' | 'margin' | 'seconds'
export type KlineFetcher = (symbol: string, resolution: string, from: number, to: number) => Promise<{ data: unknown }>

export type KlineBar = {
  timestamp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  turnover?: number
}

export function normalizeKlineModule(module: KlineModule | undefined): MarketWsModule {
  if (module === 'swap' || module === 'margin') return 'margin'
  if (module === 'second' || module === 'seconds') return 'seconds'
  return 'spot'
}

export function normalizeKlineInterval(resolution: string): string {
  const normalized = resolution.trim().toLowerCase()
  if (!normalized) return '1m'
  if (normalized.endsWith('min')) return `${Number.parseInt(normalized, 10) || 1}m`
  if (normalized === '1day') return '1d'
  return normalized
}

export function resolveKlineTopic(module: KlineModule | undefined, topic: string | undefined, symbol: string, interval: string): string {
  const normalizedInterval = normalizeKlineInterval(interval)
  if (topic) {
    return topic
      .replace(/\{symbol\}/g, symbol)
      .replace(/\{interval\}/g, normalizedInterval)
  }
  return `${normalizeKlineModule(module)}:kline:${symbol}:${normalizedInterval}`
}

export function chartSymbolValue(symbol: unknown, fallback: string): string {
  if (isRecord(symbol)) {
    const candidate = stringValue(symbol.symbol) || stringValue(symbol.ticker)
    if (candidate) return candidate
  }
  return stringValue(symbol) || fallback
}

export function klineSubscriptionKey(symbol: string, interval: string): string {
  return `${symbol}_${normalizeKlineInterval(interval)}`
}

export function historyKlineBars(data: unknown): KlineBar[] {
  if (!Array.isArray(data)) return []

  const bars = new Map<number, KlineBar>()
  for (const row of data) {
    if (!Array.isArray(row)) continue
    const bar = normalizeKlineBar({
      timestamp: row[0],
      open: row[1],
      high: row[2],
      low: row[3],
      close: row[4],
      volume: row[5]
    })
    if (bar) bars.set(bar.timestamp, bar)
  }

  return [...bars.values()].sort((left, right) => left.timestamp - right.timestamp)
}

export function parseRealtimeKline(payload: unknown): KlineBar | null {
  if (!isRecord(payload)) return null
  return normalizeKlineBar({
    timestamp: payload.time ?? payload.timestamp ?? payload.open_time,
    open: payload.openPrice ?? payload.open,
    high: payload.highestPrice ?? payload.high,
    low: payload.lowestPrice ?? payload.low,
    close: payload.closePrice ?? payload.close,
    volume: payload.volume ?? payload.vol,
    turnover: payload.turnover ?? payload.amount
  })
}

export function klineLookbackMs(interval: string, limit = 100): number {
  const normalized = normalizeKlineInterval(interval)
  const match = normalized.match(/^(\d+)(m|h|d|w)$/)
  if (!match) return limit * 60_000

  const unitMs = {
    m: 60_000,
    h: 3_600_000,
    d: 86_400_000,
    w: 604_800_000
  } as const
  return Math.max(1, Number(match[1])) * unitMs[match[2] as keyof typeof unitMs] * limit
}

function normalizeKlineBar(input: Record<string, unknown>): KlineBar | null {
  const timestamp = normalizeTimestamp(input.timestamp)
  const open = finiteNumber(input.open)
  const high = finiteNumber(input.high)
  const low = finiteNumber(input.low)
  const close = finiteNumber(input.close)
  if (!timestamp || open === null || high === null || low === null || close === null) return null
  if (open <= 0 || high <= 0 || low <= 0 || close <= 0) return null

  const volume = finiteNumber(input.volume) ?? 0
  const turnover = finiteNumber(input.turnover)
  return {
    timestamp,
    open,
    high,
    low,
    close,
    volume: Math.max(0, volume),
    ...(turnover === null ? {} : { turnover: Math.max(0, turnover) })
  }
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = finiteNumber(value)
  if (timestamp === null || timestamp <= 0) return 0
  return timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function finiteNumber(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : null
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
