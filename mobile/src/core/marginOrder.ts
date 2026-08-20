import type { MarginOrderType } from './types.ts'

/** Keeps only order types explicitly advertised by the backend capability set. */
export function parseMarginOrderTypes(value: unknown): MarginOrderType[] {
  const values = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value.split(',')
      : []
  const normalized = values
    .map((item) => String(item).trim().toLowerCase())
    .filter((item): item is MarginOrderType => item === 'market' || item === 'limit')
  return [...new Set(normalized)]
}

/** Preserves a valid selection, otherwise follows the Pencil default by preferring an advertised limit order. */
export function preferredMarginOrderType(
  current: MarginOrderType | null | undefined,
  supported: readonly MarginOrderType[],
): MarginOrderType | null {
  if (current && supported.includes(current)) return current
  if (supported.includes('limit')) return 'limit'
  return supported[0] ?? null
}

export function isFilledMarginPosition<T extends { entryPrice: number | null }>(
  position: T,
): position is T & { entryPrice: number } {
  return position.entryPrice !== null && Number.isFinite(position.entryPrice) && position.entryPrice > 0
}

export function isPendingMarginPosition(position: { entryPrice: number | null; status: string }): boolean {
  return position.status.trim().toLowerCase() === 'opened' && !isFilledMarginPosition(position)
}

/** Uses ask for long, bid for short, and the latest ticker only when that BBO side is absent. */
export function marginLimitPriceFromBbo(input: {
  side: 'buy' | 'sell'
  bids: ReadonlyArray<{ price: number }>
  asks: ReadonlyArray<{ price: number }>
  latestPrice: number
}): number | null {
  const levels = input.side === 'buy' ? input.asks : input.bids
  const prices = levels.map((level) => level.price).filter((price) => Number.isFinite(price) && price > 0)
  const bbo = prices.length
    ? input.side === 'buy' ? Math.min(...prices) : Math.max(...prices)
    : null
  if (bbo !== null) return bbo
  return Number.isFinite(input.latestPrice) && input.latestPrice > 0 ? input.latestPrice : null
}
