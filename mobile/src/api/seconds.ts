import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'
import { mapSecondsOrder, type SecondsOrder } from '@/core/secondsOrder'

export { mapSecondsOrder }
export type { SecondsOrder }

export interface SecondsCycle {
  id: number
  durationSeconds: number
  payoutRate: number
  minStake: number
  maxStake?: number
}

export interface SecondsProduct {
  id: number
  symbol: string
  stakeAssetId: number
  stakeAssetSymbol: string
  cycles: SecondsCycle[]
  status: string
}

export async function fetchSecondsProducts(limit = 50): Promise<SecondsProduct[]> {
  const response = await client.get<{ products?: Array<Record<string, unknown>> }>(requestUrl('/seconds-contracts/products'), { params: { limit } })
  return (response.data.products || []).map((product) => {
    const cycles = Array.isArray(product.cycles) ? product.cycles : []
    const mappedCycles = cycles.map((cycle) => mapCycle(cycle as Record<string, unknown>))
    if (!mappedCycles.length) {
      mappedCycles.push({
        id: asNumber(product.id),
        durationSeconds: asNumber(product.duration_seconds),
        payoutRate: asNumber(product.payout_rate),
        minStake: asNumber(product.min_stake),
        maxStake: product.max_stake === null || product.max_stake === undefined ? undefined : asNumber(product.max_stake),
      })
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
}

export async function fetchSecondsOrders(limit = 50): Promise<SecondsOrder[]> {
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/seconds-contracts/orders'), { params: { limit } })
  return (response.data.orders || []).map(mapSecondsOrder)
}

export interface OpenSecondsOrderInput {
  productId: number
  durationSeconds: number
  direction: 'up' | 'down'
  stakeAmount: number
  idempotencyKey?: string
}

export async function openSecondsOrder(input: OpenSecondsOrderInput): Promise<SecondsOrder> {
  const response = await client.post<{ order?: Record<string, unknown> }>(requestUrl('/seconds-contracts/orders'), {
    product_id: input.productId,
    duration_seconds: input.durationSeconds,
    direction: input.direction,
    stake_amount: String(input.stakeAmount),
    idempotency_key: input.idempotencyKey || createSecondsOrderIdempotencyKey(),
  })
  if (!response.data.order) throw new Error('Seconds-contract order response is missing order data')
  return mapSecondsOrder(response.data.order)
}

export function createSecondsOrderIdempotencyKey(): string {
  return createIdempotencyKey('mobile-seconds')
}

function mapCycle(cycle: Record<string, unknown>): SecondsCycle {
  return {
    id: asNumber(cycle.id),
    durationSeconds: asNumber(cycle.duration_seconds),
    payoutRate: asNumber(cycle.payout_rate),
    minStake: asNumber(cycle.min_stake),
    maxStake: cycle.max_stake === null || cycle.max_stake === undefined ? undefined : asNumber(cycle.max_stake),
  }
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
