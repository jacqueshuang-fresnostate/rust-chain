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

export interface SecondsOrderProfitLossPresentation {
  translationKey: 'seconds.profitAmount' | 'seconds.lossAmount' | 'seconds.profitLossAmount'
  amount?: number
  tone: 'positive' | 'negative' | 'pending'
}

export type SecondsHistoryDirectionFilter = 'all' | 'up' | 'down'

export interface SecondsSettlementResultTracker {
  track: (order: SecondsOrder) => void
  reconcile: (orders: readonly SecondsOrder[]) => SecondsOrder[]
  isTracking: (orderId: number) => boolean
  reset: () => void
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

/**
 * Filter an already-authoritative history snapshot without mutating its order.
 * The `all` branch also returns a copy so callers never receive the source array.
 */
export function filterSecondsHistoryOrdersByDirection(
  orders: readonly SecondsOrder[],
  direction: SecondsHistoryDirectionFilter,
): SecondsOrder[] {
  if (direction === 'all') return [...orders]
  return orders.filter((order) => order.direction === direction)
}

/**
 * 创建仅存活于当前页面会话的结算结果追踪器。
 *
 * 首次调和时先检查“上一个权威快照已观察为活动”的订单，再把本次新出现的
 * 活动订单纳入基线。这个顺序保证首次进页时的历史输赢不会被补提示，同时
 * 允许“活动 -> 非活动但结果暂缺 -> win/loss”的延迟结算链路。
 */
export function createSecondsSettlementResultTracker(): SecondsSettlementResultTracker {
  const trackedActiveOrderIds = new Set<number>()
  const handledOrderIds = new Set<number>()

  return {
    track(order: SecondsOrder): void {
      if (isActiveSecondsOrder(order) && !handledOrderIds.has(order.id)) {
        trackedActiveOrderIds.add(order.id)
      }
    },
    reconcile(nextOrders: readonly SecondsOrder[]): SecondsOrder[] {
      const resolvedOrders: SecondsOrder[] = []

      // 只从这次后端列表快照读取结果；倒计时、行情和上一次本地列表都不参与输赢判定。
      for (const order of nextOrders) {
        if (!trackedActiveOrderIds.has(order.id) || handledOrderIds.has(order.id)) continue
        if (isCancelledSecondsOrder(order)) {
          trackedActiveOrderIds.delete(order.id)
          // 取消是本页会话的终态。即使之后收到乱序的旧活动快照，
          // 也不能重新追踪并把该订单误报为输赢。
          handledOrderIds.add(order.id)
          continue
        }
        if (isActiveSecondsOrder(order) || !isSettledSecondsOrder(order)) continue

        const result = normalizedSecondsOrderResult(order.result)
        if (!result) {
          // 非活动但暂无结果时保留追踪资格，后续轮询补齐结果仍能通知。
          continue
        }

        handledOrderIds.add(order.id)
        trackedActiveOrderIds.delete(order.id)
        resolvedOrders.push(order)
      }

      // 必须放在结果检查之后：本次列表中首次出现的订单只建立基线，不产生历史补弹。
      for (const order of nextOrders) {
        if (isActiveSecondsOrder(order) && !handledOrderIds.has(order.id)) {
          trackedActiveOrderIds.add(order.id)
        }
      }

      return resolvedOrders.sort(compareSecondsSettlementResultOrder)
    },
    isTracking(orderId: number): boolean {
      return trackedActiveOrderIds.has(orderId)
    },
    reset(): void {
      trackedActiveOrderIds.clear()
      handledOrderIds.clear()
    },
  }
}

/**
 * 将新结算结果追加到待展示队尾。队首始终保持当前卡片，重试或重复快照
 * 中的同一订单 ID 不会挤入第二份卡片。
 */
export function enqueueSecondsSettlementResults(
  queue: readonly SecondsOrder[],
  nextResults: readonly SecondsOrder[],
): SecondsOrder[] {
  const queuedOrderIds = new Set(queue.map((order) => order.id))
  const merged = [...queue]
  for (const order of nextResults) {
    if (queuedOrderIds.has(order.id)) continue
    queuedOrderIds.add(order.id)
    merged.push(order)
  }
  return merged
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

/**
 * 只根据下单时固化的本金、赔率和最终结果生成历史盈亏展示值。
 * 赢单返回净收益而非含本金的总派彩，输单返回负本金；没有权威结果时保持不可用。
 */
export function secondsOrderProfitLossPresentation(
  order: Pick<SecondsOrder, 'result' | 'stakeAmount' | 'payoutRate'>,
): SecondsOrderProfitLossPresentation {
  const result = normalizedSecondsOrderResult(order.result)
  if (result === 'win') {
    const amount = secondsOrderEstimatedProfit(order)
    return {
      translationKey: 'seconds.profitAmount',
      amount: Number.isFinite(amount) && amount >= 0 ? amount : undefined,
      tone: Number.isFinite(amount) && amount >= 0 ? 'positive' : 'pending',
    }
  }
  if (result === 'loss') {
    const amount = -Math.abs(order.stakeAmount)
    return {
      translationKey: 'seconds.lossAmount',
      amount: Number.isFinite(amount) ? amount : undefined,
      tone: Number.isFinite(amount) ? 'negative' : 'pending',
    }
  }
  return {
    translationKey: 'seconds.profitLossAmount',
    amount: undefined,
    tone: 'pending',
  }
}

function normalizedSecondsOrderResult(value: string | undefined): 'win' | 'loss' | null {
  const result = value?.trim().toLowerCase()
  return result === 'win' || result === 'loss' ? result : null
}

function isCancelledSecondsOrder(order: Pick<SecondsOrder, 'status'>): boolean {
  const status = order.status.trim().toLowerCase()
  return status === 'cancelled' || status === 'canceled'
}

function isSettledSecondsOrder(order: Pick<SecondsOrder, 'status'>): boolean {
  return order.status.trim().toLowerCase() === 'settled'
}

function compareSecondsSettlementResultOrder(left: SecondsOrder, right: SecondsOrder): number {
  const leftExpiry = Number.isFinite(left.expiresAt) ? left.expiresAt : Number.POSITIVE_INFINITY
  const rightExpiry = Number.isFinite(right.expiresAt) ? right.expiresAt : Number.POSITIVE_INFINITY
  return leftExpiry - rightExpiry || left.id - right.id
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

export function secondsOrderEstimatedProfit(
  order: Pick<SecondsOrder, 'stakeAmount' | 'payoutRate'>,
): number {
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
