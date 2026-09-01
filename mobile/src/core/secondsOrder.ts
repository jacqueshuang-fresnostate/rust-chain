import { asNumber } from './format.ts'
import {
  decimalMultiply,
  decimalNegate,
  requiredDecimalText,
  type DecimalText,
} from './decimal.ts'

const SECONDS_DECIMAL_CONSTRAINTS = {
  maxIntegerDigits: 20,
  maxScale: 18,
} as const

export class SecondsOrderContractError extends TypeError {
  constructor(field: string) {
    super(`invalid seconds order ${field}`)
    this.name = 'SecondsOrderContractError'
  }
}

export class SecondsHistoryPageContractError extends TypeError {
  constructor(field: string) {
    super(`invalid seconds orders page ${field}`)
    this.name = 'SecondsHistoryPageContractError'
  }
}

export interface SecondsOrder {
  id: number
  symbol: string
  stakeAssetSymbol: string
  direction: 'up' | 'down'
  stakeAmount: number
  stakeAmountText: DecimalText
  durationSeconds: number
  payoutRate: number
  payoutRateText: DecimalText
  entryPrice?: number
  entryPriceText: DecimalText | null
  settlementPrice?: number
  settlementPriceText: DecimalText | null
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
  /** Exact authority for presentation; `amount` only supports legacy terminal rendering. */
  amountText: DecimalText | null
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

export interface SecondsHistoryPageRequest {
  limit: number
  offset: number
}

export interface SecondsHistoryPage {
  orders: SecondsOrder[]
  nextOffset: number
  hasMore: boolean
}

export interface SecondsHistoryPageMerge {
  orders: SecondsOrder[]
  nextOffset: number
  hasMore: boolean
  addedCount: number
}

export interface SecondsHistoryPaginationState {
  orders: SecondsOrder[]
  loading: boolean
  loadingMore: boolean
  nextOffset: number
  hasMore: boolean
  initialError: unknown | null
  appendError: unknown | null
}

export type SecondsHistoryPaginationOperation =
  | 'loaded'
  | 'error'
  | 'guest'
  | 'stale'
  | 'ignored'

export interface SecondsHistoryPaginationController {
  snapshot: () => SecondsHistoryPaginationState
  loadInitial: () => Promise<SecondsHistoryPaginationOperation>
  loadMore: () => Promise<SecondsHistoryPaginationOperation>
  retryLoadMore: () => Promise<SecondsHistoryPaginationOperation>
  reset: () => void
  stop: () => void
}

export type SecondsHistoryRequestResult =
  | { state: 'loaded'; page: SecondsHistoryPage }
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
    .sort(compareSecondsHistoryOrder)
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

function compareSecondsHistoryOrder(left: SecondsOrder, right: SecondsOrder): number {
  return right.createdAt - left.createdAt || right.id - left.id
}

/**
 * Merge one raw server page by order ID without changing the page offset.
 * A later row replaces the earlier snapshot for the same ID, then the merged
 * snapshot is restored to the backend's `created_at DESC, id DESC` order.
 * `addedCount` counts only IDs not seen before.
 */
export function mergeSecondsHistoryOrderPage(
  currentOrders: readonly SecondsOrder[],
  page: SecondsHistoryPage,
): SecondsHistoryPageMerge {
  const orders: SecondsOrder[] = []
  const orderIndex = new Map<number, number>()

  const upsert = (order: SecondsOrder): boolean => {
    const existingIndex = orderIndex.get(order.id)
    if (existingIndex !== undefined) {
      orders[existingIndex] = order
      return false
    }
    orderIndex.set(order.id, orders.length)
    orders.push(order)
    return true
  }

  for (const order of currentOrders) upsert(order)
  let addedCount = 0
  for (const order of page.orders) {
    if (upsert(order)) addedCount += 1
  }

  orders.sort(compareSecondsHistoryOrder)

  return {
    orders,
    nextOffset: page.nextOffset,
    hasMore: page.hasMore && page.orders.length > 0 && addedCount > 0,
    addedCount,
  }
}

/** Map the paginated transport envelope and preserve raw-row offset progress. */
export function mapSecondsHistoryPage(
  payload: unknown,
  request: SecondsHistoryPageRequest,
): SecondsHistoryPage {
  if (!Number.isSafeInteger(request.limit) || request.limit < 1 || request.limit > 100) {
    throw new SecondsHistoryPageContractError('limit')
  }
  if (!Number.isSafeInteger(request.offset) || request.offset < 0) {
    throw new SecondsHistoryPageContractError('offset')
  }
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    throw new SecondsHistoryPageContractError('payload')
  }

  const response = payload as Record<string, unknown>
  const rawOrders = response.orders
  if (!Array.isArray(rawOrders)) throw new SecondsHistoryPageContractError('orders')
  if (rawOrders.length > request.limit) {
    throw new SecondsHistoryPageContractError('orders length')
  }
  if (response.has_more !== undefined && typeof response.has_more !== 'boolean') {
    throw new SecondsHistoryPageContractError('has_more')
  }

  const orders = rawOrders.map((order) => {
    if (!order || typeof order !== 'object' || Array.isArray(order)) {
      throw new SecondsHistoryPageContractError('orders item')
    }
    const row = order as Record<string, unknown>
    if (!Number.isSafeInteger(row.id) || Number(row.id) <= 0) {
      throw new SecondsHistoryPageContractError('orders item id')
    }
    if (!isPositiveSafeTimestamp(row.created_at)) {
      throw new SecondsHistoryPageContractError('orders item created_at')
    }
    return mapSecondsOrder(row)
  })
  return {
    orders,
    nextOffset: request.offset + rawOrders.length,
    hasMore: response.has_more === undefined
      ? rawOrders.length === request.limit
      : response.has_more,
  }
}

function isPositiveSafeTimestamp(value: unknown): boolean {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

/** Create a detached snapshot so view state cannot mutate controller authority. */
export function createSecondsHistoryPaginationState(): SecondsHistoryPaginationState {
  return {
    orders: [],
    loading: false,
    loadingMore: false,
    nextOffset: 0,
    hasMore: false,
    initialError: null,
    appendError: null,
  }
}

/**
 * Own the initial/append state split and the one-page-at-a-time guard.
 * The view only forwards observer/retry intents, while this controller keeps
 * offsets, stale-session suppression, de-duplication, and append recovery in
 * one behaviorally testable lifecycle.
 */
export function createSecondsHistoryPaginationController(options: {
  sessionToken: () => string
  sessionGeneration: () => number
  fetchPage: (request: SecondsHistoryPageRequest) => Promise<SecondsHistoryPage>
  pageSize?: number
  onChange?: (state: SecondsHistoryPaginationState) => void
}): SecondsHistoryPaginationController {
  const pageSize = options.pageSize ?? 20
  if (!Number.isSafeInteger(pageSize) || pageSize < 1 || pageSize > 100) {
    throw new SecondsHistoryPageContractError('limit')
  }

  const requests = createSecondsHistoryRequestLifecycle(options)
  let state = createSecondsHistoryPaginationState()
  let active = true

  const snapshot = (): SecondsHistoryPaginationState => ({
    ...state,
    orders: [...state.orders],
  })
  const publish = (): void => options.onChange?.(snapshot())
  const replaceState = (next: SecondsHistoryPaginationState): void => {
    state = next
    publish()
  }

  async function loadInitial(): Promise<SecondsHistoryPaginationOperation> {
    if (!active) return 'stale'
    if (!options.sessionToken()) {
      requests.invalidate()
      replaceState(createSecondsHistoryPaginationState())
      return 'guest'
    }
    replaceState({
      ...createSecondsHistoryPaginationState(),
      loading: true,
    })

    const result = await requests.load({ limit: pageSize, offset: 0 })
    if (result.state === 'stale') return 'stale'
    if (result.state === 'guest') {
      replaceState(createSecondsHistoryPaginationState())
      return 'guest'
    }
    if (result.state === 'error') {
      replaceState({
        ...createSecondsHistoryPaginationState(),
        initialError: normalizedSecondsHistoryRequestError(result.error),
      })
      return 'error'
    }

    const merged = mergeSecondsHistoryOrderPage([], result.page)
    replaceState({
      ...createSecondsHistoryPaginationState(),
      orders: merged.orders,
      nextOffset: merged.nextOffset,
      hasMore: merged.hasMore,
    })
    return 'loaded'
  }

  async function append(retry: boolean): Promise<SecondsHistoryPaginationOperation> {
    if (!active) return 'stale'
    if (!options.sessionToken()) {
      requests.invalidate()
      replaceState(createSecondsHistoryPaginationState())
      return 'guest'
    }
    if (
      state.loading
      || state.loadingMore
      || !state.hasMore
      || (retry ? state.appendError === null : state.appendError !== null)
    ) return 'ignored'

    const offset = state.nextOffset
    replaceState({
      ...state,
      loadingMore: true,
    })

    const result = await requests.load({ limit: pageSize, offset })
    if (result.state === 'stale') return 'stale'
    if (result.state === 'guest') {
      replaceState(createSecondsHistoryPaginationState())
      return 'guest'
    }
    if (result.state === 'error') {
      replaceState({
        ...state,
        loadingMore: false,
        appendError: normalizedSecondsHistoryRequestError(result.error),
      })
      return 'error'
    }

    const merged = mergeSecondsHistoryOrderPage(state.orders, result.page)
    replaceState({
      ...state,
      orders: merged.orders,
      loadingMore: false,
      nextOffset: merged.nextOffset,
      hasMore: merged.hasMore,
      appendError: null,
    })
    return 'loaded'
  }

  return {
    snapshot,
    loadInitial,
    loadMore: () => append(false),
    retryLoadMore: () => append(true),
    reset(): void {
      requests.invalidate()
      replaceState(createSecondsHistoryPaginationState())
    },
    stop(): void {
      active = false
      requests.stop()
    },
  }
}

function normalizedSecondsHistoryRequestError(error: unknown): unknown {
  return error ?? new Error('seconds history request failed')
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
  order: Pick<SecondsOrder, 'result' | 'stakeAmountText' | 'payoutRateText'>,
): SecondsOrderProfitLossPresentation {
  const result = normalizedSecondsOrderResult(order.result)
  if (result === 'win') {
    const amountText = secondsOrderEstimatedProfit(order)
    const amount = amountText ? decimalDisplayNumber(amountText) : undefined
    return {
      translationKey: 'seconds.profitAmount',
      amountText,
      amount,
      tone: amountText ? 'positive' : 'pending',
    }
  }
  if (result === 'loss') {
    const stakeAmountText = exactSecondsOrderDecimal(order.stakeAmountText)
    const amountText = stakeAmountText ? decimalNegate(stakeAmountText) : null
    const amount = amountText ? decimalDisplayNumber(amountText) : undefined
    return {
      translationKey: 'seconds.lossAmount',
      amountText,
      amount,
      tone: amountText ? 'negative' : 'pending',
    }
  }
  return {
    translationKey: 'seconds.profitLossAmount',
    amountText: null,
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
  sessionToken: () => string
  sessionGeneration: () => number
  fetchPage: (request: SecondsHistoryPageRequest) => Promise<SecondsHistoryPage>
}): {
  load: (request: SecondsHistoryPageRequest) => Promise<SecondsHistoryRequestResult>
  invalidate: () => void
  stop: () => void
} {
  let active = true
  let generation = 0

  return {
    async load(request: SecondsHistoryPageRequest): Promise<SecondsHistoryRequestResult> {
      const requestGeneration = ++generation
      if (!active) return { state: 'stale' }
      const sessionToken = options.sessionToken()
      const sessionGeneration = options.sessionGeneration()
      if (!sessionToken) return { state: 'guest' }

      try {
        const page = await options.fetchPage({ limit: request.limit, offset: request.offset })
        if (
          !active
          || requestGeneration !== generation
          || options.sessionToken() !== sessionToken
          || options.sessionGeneration() !== sessionGeneration
        ) {
          return { state: 'stale' }
        }
        return { state: 'loaded', page }
      } catch (error) {
        if (
          !active
          || requestGeneration !== generation
          || options.sessionToken() !== sessionToken
          || options.sessionGeneration() !== sessionGeneration
        ) {
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
  order: Pick<SecondsOrder, 'stakeAmountText' | 'payoutRateText'>,
): DecimalText | null {
  const stakeAmountText = exactSecondsOrderDecimal(order.stakeAmountText)
  const payoutRateText = exactSecondsOrderDecimal(order.payoutRateText)
  return stakeAmountText && payoutRateText
    ? decimalMultiply(stakeAmountText, payoutRateText)
    : null
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
  const direction = secondsDirection(order.direction)
  const stakeAmountText = secondsOrderDecimal(order.stake_amount, 'stake_amount', false)
  const payoutRateText = secondsOrderDecimal(order.payout_rate, 'payout_rate', false)
  const entryPriceText = nullableSecondsOrderDecimal(order.entry_price, 'entry_price', false)
  const settlementPriceText = nullableSecondsOrderDecimal(
    order.settlement_price,
    'settlement_price',
    false,
  )
  return {
    id: asNumber(order.id),
    symbol: String(order.symbol || ''),
    stakeAssetSymbol: String(order.stake_asset_symbol || '').toUpperCase(),
    direction,
    stakeAmount: decimalDisplayNumber(stakeAmountText),
    stakeAmountText,
    durationSeconds: asNumber(order.duration_seconds),
    payoutRate: decimalDisplayNumber(payoutRateText),
    payoutRateText,
    entryPrice: entryPriceText ? decimalDisplayNumber(entryPriceText) : undefined,
    entryPriceText,
    settlementPrice: settlementPriceText ? decimalDisplayNumber(settlementPriceText) : undefined,
    settlementPriceText,
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

function secondsOrderDecimal(value: unknown, field: string, allowZero: boolean): DecimalText {
  try {
    return requiredDecimalText(value, field, 'seconds order', {
      ...SECONDS_DECIMAL_CONSTRAINTS,
      allowNegative: false,
      allowZero,
    })
  } catch {
    throw new SecondsOrderContractError(field)
  }
}

function nullableSecondsOrderDecimal(
  value: unknown,
  field: string,
  allowZero: boolean,
): DecimalText | null {
  if (value === null || value === undefined) return null
  return secondsOrderDecimal(value, field, allowZero)
}

function exactSecondsOrderDecimal(value: unknown): DecimalText | null {
  try {
    return requiredDecimalText(value, 'financial authority', 'seconds order', {
      ...SECONDS_DECIMAL_CONSTRAINTS,
      allowNegative: false,
    })
  } catch {
    return null
  }
}

function decimalDisplayNumber(value: DecimalText): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
