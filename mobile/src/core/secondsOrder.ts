import { asNumber } from './format.ts'

export interface SecondsOrder {
  id: number
  symbol: string
  stakeAssetSymbol: string
  direction: 'up' | 'down'
  stakeAmount: number
  durationSeconds: number
  payoutRate: number
  entryPrice?: number
  settlementPrice?: number
  status: string
  result?: string
  expiresAt: number
  createdAt: number
}

export interface SecondsOrderStatusPresentation {
  translationKey?: string
  source: string
  tone: 'positive' | 'negative' | 'pending'
}

export type SecondsHistoryRequestResult =
  | { state: 'loaded'; orders: SecondsOrder[] }
  | { state: 'error'; error: unknown }
  | { state: 'guest' }
  | { state: 'stale' }

const ACTIVE_SECONDS_ORDER_STATUSES = new Set(['opened', 'pending', 'active'])

export function isActiveSecondsOrder(order: SecondsOrder): boolean {
  return ACTIVE_SECONDS_ORDER_STATUSES.has(order.status.trim().toLowerCase())
}

export function activeSecondsOrders(orders: readonly SecondsOrder[]): SecondsOrder[] {
  return orders.filter(isActiveSecondsOrder)
}

export function historicalSecondsOrders(orders: readonly SecondsOrder[]): SecondsOrder[] {
  return orders
    .filter((order) => !isActiveSecondsOrder(order))
    .sort((left, right) => right.createdAt - left.createdAt)
}

export function secondsOrderStatusPresentation(
  order: Pick<SecondsOrder, 'result' | 'status'>,
): SecondsOrderStatusPresentation {
  const resultSource = order.result?.trim()
  const result = resultSource?.toLowerCase()
  if (result === 'win') {
    return { translationKey: 'seconds.statusWon', source: resultSource || 'win', tone: 'positive' }
  }
  if (result === 'loss') {
    return { translationKey: 'seconds.statusLost', source: resultSource || 'loss', tone: 'negative' }
  }
  if (resultSource) {
    return { translationKey: undefined, source: resultSource, tone: 'pending' }
  }

  const statusSource = order.status.trim()
  const status = statusSource.toLowerCase()
  const translationKeys: Record<string, string> = {
    opened: 'seconds.statusActive',
    pending: 'seconds.statusPending',
    active: 'seconds.statusActive',
    won: 'seconds.statusWon',
    lost: 'seconds.statusLost',
    settled: 'seconds.statusSettled',
    cancelled: 'seconds.statusCancelled',
    canceled: 'seconds.statusCancelled',
  }
  const tone = status === 'won'
    ? 'positive'
    : status === 'lost' || status === 'cancelled' || status === 'canceled'
      ? 'negative'
      : 'pending'
  return {
    translationKey: translationKeys[status],
    source: statusSource,
    tone,
  }
}

export function createSecondsHistoryRequestLifecycle(options: {
  isAuthenticated: () => boolean
  fetchOrders: (limit: number) => Promise<SecondsOrder[]>
  limit?: number
}): {
  load: () => Promise<SecondsHistoryRequestResult>
  invalidate: () => void
  stop: () => void
} {
  let active = true
  let generation = 0

  return {
    async load(): Promise<SecondsHistoryRequestResult> {
      const requestGeneration = ++generation
      if (!active) return { state: 'stale' }
      if (!options.isAuthenticated()) return { state: 'guest' }

      try {
        const orders = await options.fetchOrders(options.limit ?? 100)
        if (!active || requestGeneration !== generation || !options.isAuthenticated()) {
          return { state: 'stale' }
        }
        return { state: 'loaded', orders }
      } catch (error) {
        if (!active || requestGeneration !== generation || !options.isAuthenticated()) {
          return { state: 'stale' }
        }
        return { state: 'error', error }
      }
    },
    invalidate(): void {
      generation += 1
    },
    stop(): void {
      active = false
      generation += 1
    },
  }
}

export function secondsOrderRemainingMs(order: SecondsOrder, now: number): number {
  return Math.max(0, order.expiresAt - now)
}

export function secondsOrderProgress(order: SecondsOrder, now: number): number {
  const duration = order.expiresAt - order.createdAt
  if (duration <= 0) return 0
  return Math.max(0, Math.min(100, ((now - order.createdAt) / duration) * 100))
}

export function secondsOrderEstimatedProfit(order: SecondsOrder): number {
  return order.stakeAmount * order.payoutRate
}

export function upsertSecondsOrder(
  orders: readonly SecondsOrder[],
  nextOrder: SecondsOrder,
): SecondsOrder[] {
  return [nextOrder, ...orders.filter((order) => order.id !== nextOrder.id)]
}

/**
 * Reconciles a server list without discarding a create response that the
 * list endpoint has not observed yet. A matching server row remains
 * authoritative, including a transition from opened to settled.
 */
export function mergeSecondsOrderReconciliation(
  serverOrders: readonly SecondsOrder[],
  committedOrders: readonly SecondsOrder[],
): SecondsOrder[] {
  const serverIds = new Set(serverOrders.map((order) => order.id))
  return [
    ...committedOrders.filter((order) => !serverIds.has(order.id)),
    ...serverOrders,
  ]
}

export function mapSecondsOrder(order: Record<string, unknown>): SecondsOrder {
  return {
    id: asNumber(order.id),
    symbol: String(order.symbol || ''),
    stakeAssetSymbol: String(order.stake_asset_symbol || '').toUpperCase(),
    direction: secondsDirection(order.direction),
    stakeAmount: asNumber(order.stake_amount),
    durationSeconds: asNumber(order.duration_seconds),
    payoutRate: asNumber(order.payout_rate),
    entryPrice: optionalNumber(order.entry_price),
    settlementPrice: optionalNumber(order.settlement_price),
    status: String(order.status || ''),
    result: optionalText(order.result),
    expiresAt: normalizeTimestamp(order.expires_at),
    createdAt: normalizeTimestamp(order.created_at),
  }
}

function secondsDirection(value: unknown): SecondsOrder['direction'] {
  const direction = String(value || '').trim().toLowerCase()
  if (direction === 'up' || direction === 'down') return direction
  throw new Error('Seconds-contract order response contains an invalid direction')
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function optionalNumber(value: unknown): number | undefined {
  if (value === null || value === undefined || value === '') return undefined
  if (typeof value !== 'number' && typeof value !== 'string') return undefined
  const number = Number(value)
  return Number.isFinite(number) ? number : undefined
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
