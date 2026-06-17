import request from './request'
import { backendApiUrl } from './backendAdapters'
import { formatBusinessOrderNo } from '@/utils/orderNo'

export type PredictionOutcome = 'yes' | 'no'
export type PredictionOrderStatus = 'open' | 'settled' | 'refunded'

export interface PredictionStakeAsset {
  asset_id: number
  asset_symbol: string
  max_payout_amount: string
}

export interface PredictionConfig {
  allowed_assets: PredictionStakeAsset[]
  default_fee_rate: string
  quote_ttl_seconds: number
}

export interface PredictionMarket {
  id: number
  title: string
  description?: string | null
  image_url?: string | null
  category?: string | null
  outcome_yes_label: string
  outcome_no_label: string
  yes_price: string
  no_price: string
  volume?: string | null
  liquidity?: string | null
  end_at?: number | null
  display_status: string
  settlement_status: string
  external_resolution?: string | null
  local_resolution?: string | null
  allowed_asset_ids_override_json?: Array<string | number> | null
  fee_rate_override?: string | null
  last_synced_at?: number | null
}

export interface PredictionQuote {
  quote_id: string
  market_id: number
  outcome: PredictionOutcome
  asset_id: number
  asset_symbol: string
  stake_amount: string
  fee_amount: string
  accepted_price: string
  shares: string
  theoretical_payout: string
  effective_payout_cap: string
  expires_at: number
}

export interface PredictionOrder {
  id: number
  order_no?: string | null
  orderNo: string
  market_id: number
  market_title: string
  outcome: string
  asset_symbol: string
  stake_amount: string
  fee_amount: string
  accepted_price: string
  shares: string
  theoretical_payout: string
  effective_payout_cap: string
  status: PredictionOrderStatus
  result?: string | null
  payout_amount: string
  refund_amount: string
  fee_refund_amount: string
  invalid_refund_policy_used?: string | null
  settled_at?: number | null
  created_at: number
}

export interface CreatePredictionQuotePayload {
  market_id: number
  outcome: PredictionOutcome
  asset_id: number
  stake_amount: string
}

export interface CreatePredictionOrderPayload {
  quote_id: string
  idempotency_key: string
}

function normalizeOrder(order: Omit<PredictionOrder, 'orderNo'>): PredictionOrder {
  return {
    ...order,
    orderNo: formatBusinessOrderNo('PM', order as unknown as Record<string, unknown>),
  }
}

export async function fetchPredictionConfig() {
  const res = await request.instance.get<PredictionConfig>(backendApiUrl('/prediction/config'))
  return { data: res.data }
}

export async function fetchPredictionMarkets(limit = 100) {
  const res = await request.instance.get<{ markets: PredictionMarket[] }>(backendApiUrl(`/prediction/markets?limit=${limit}`))
  return { data: res.data.markets }
}

export async function fetchPredictionMarket(id: number) {
  const res = await request.instance.get<PredictionMarket>(backendApiUrl(`/prediction/markets/${id}`))
  return { data: res.data }
}

export async function createPredictionQuote(payload: CreatePredictionQuotePayload) {
  const res = await request.instance.post<PredictionQuote>(backendApiUrl('/prediction/quotes'), payload)
  return { data: res.data }
}

export async function createPredictionOrder(payload: CreatePredictionOrderPayload) {
  const res = await request.instance.post<{ order: Omit<PredictionOrder, 'orderNo'>; changed: boolean }>(backendApiUrl('/prediction/orders'), payload)
  return { data: { order: normalizeOrder(res.data.order), changed: res.data.changed } }
}

export async function fetchPredictionOrders(status?: PredictionOrderStatus | '') {
  const query = status ? `?status=${encodeURIComponent(status)}` : ''
  const res = await request.instance.get<{ orders: Array<Omit<PredictionOrder, 'orderNo'>> }>(backendApiUrl(`/prediction/orders${query}`))
  return { data: res.data.orders.map(normalizeOrder) }
}
