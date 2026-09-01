import { client, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { asNumber } from '@/core/format'
import {
  mapSecondsHistoryPage,
  mapSecondsOrder,
  type SecondsHistoryPage,
  type SecondsHistoryPageRequest,
  type SecondsOrder,
} from '@/core/secondsOrder'
import { normalizeDecimalText, requiredDecimalText, type DecimalText } from '@/core/decimal'

const SECONDS_PRODUCT_DECIMAL_CONSTRAINTS = {
  allowNegative: false,
  allowZero: false,
  maxIntegerDigits: 20,
  maxScale: 18,
} as const

export class SecondsContractError extends TypeError {
  constructor(field: string) {
    super(`invalid seconds product ${field}`)
    this.name = 'SecondsContractError'
  }
}

export { mapSecondsOrder }
export type { SecondsOrder }

export interface SecondsCycle {
  id: number
  durationSeconds: number
  payoutRate: number
  payoutRateText: DecimalText
  minStake: number
  maxStake?: number
  minStakeText: DecimalText
  maxStakeText: DecimalText | null
}

export interface SecondsProduct {
  id: number
  symbol: string
  stakeAssetId: number
  stakeAssetSymbol: string
  cycles: SecondsCycle[]
  status: string
}

export async function fetchSecondsProducts(limit = 50, options: ReferenceRequestOptions = {}): Promise<SecondsProduct[]> {
  const url = requestUrl('/seconds-contracts/products')
  return referenceRequestRegistry.request(createReferenceRequestKey(url, { limit }), 60_000, async () => {
    const response = await client.get<{ products?: Array<Record<string, unknown>> }>(url, { params: { limit } })
    return (response.data.products || []).map((product) => {
      const cycles = Array.isArray(product.cycles) ? product.cycles : []
      const mappedCycles = cycles.map((cycle) => mapSecondsCycle(cycle as Record<string, unknown>))
      if (!mappedCycles.length) {
        mappedCycles.push(mapSecondsCycle(product))
      }
      return {
        id: asNumber(product.id),
        symbol: String(product.symbol || ''),
        stakeAssetId: asNumber(product.stake_asset),
        stakeAssetSymbol: String(product.stake_asset_symbol || '').toUpperCase(),
        cycles: mappedCycles,
        status: String(product.status || ''),
      }
    })
  }, options)
}

export async function fetchSecondsOrders(limit = 50): Promise<SecondsOrder[]> {
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/seconds-contracts/orders'), { params: { limit } })
  return (response.data.orders || []).map(mapSecondsOrder)
}

export async function fetchSecondsOrdersPage(
  request: SecondsHistoryPageRequest,
): Promise<SecondsHistoryPage> {
  const response = await client.get<unknown>(requestUrl('/seconds-contracts/orders'), {
    params: { limit: request.limit, offset: request.offset },
  })
  return mapSecondsHistoryPage(response.data, request)
}

export interface OpenSecondsOrderInput {
  productId: number
  durationSeconds: number
  direction: 'up' | 'down'
  stakeAmount: DecimalText
  idempotencyKey?: string
}

export async function openSecondsOrder(input: OpenSecondsOrderInput): Promise<SecondsOrder> {
  const response = await client.post<{ order?: Record<string, unknown> }>(requestUrl('/seconds-contracts/orders'), {
    product_id: input.productId,
    duration_seconds: input.durationSeconds,
    direction: input.direction,
    stake_amount: normalizeDecimalText(input.stakeAmount),
    idempotency_key: input.idempotencyKey || createSecondsOrderIdempotencyKey(),
  })
  if (!response.data.order) throw new Error('Seconds-contract order response is missing order data')
  return mapSecondsOrder(response.data.order)
}

export function createSecondsOrderIdempotencyKey(): string {
  return createIdempotencyKey('mobile-seconds')
}

export function mapSecondsCycle(cycle: Record<string, unknown>): SecondsCycle {
  const payoutRateText = productDecimal(cycle.payout_rate, 'payout_rate')
  const minStakeText = productDecimal(cycle.min_stake, 'min_stake')
  const maxStakeText = nullableProductDecimal(cycle.max_stake, 'max_stake')
  return {
    id: asNumber(cycle.id),
    durationSeconds: asNumber(cycle.duration_seconds),
    payoutRate: decimalDisplayNumber(payoutRateText),
    payoutRateText,
    minStake: decimalDisplayNumber(minStakeText),
    maxStake: maxStakeText ? decimalDisplayNumber(maxStakeText) : undefined,
    minStakeText,
    maxStakeText,
  }
}

function productDecimal(value: unknown, field: string): DecimalText {
  try {
    return requiredDecimalText(value, field, 'seconds product', SECONDS_PRODUCT_DECIMAL_CONSTRAINTS)
  } catch {
    throw new SecondsContractError(field)
  }
}

function nullableProductDecimal(value: unknown, field: string): DecimalText | null {
  if (value === null || value === undefined) return null
  return productDecimal(value, field)
}

function decimalDisplayNumber(value: DecimalText): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) throw new SecondsContractError('decimal display value')
  return parsed
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
