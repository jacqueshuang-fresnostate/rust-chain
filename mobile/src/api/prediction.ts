import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'
import { i18n } from '@/i18n'

export type PredictionOutcome = 'yes' | 'no'

export interface PredictionAsset {
  assetId: number
  assetSymbol: string
  maxPayoutAmount: number
}

export interface PredictionMarket {
  id: number
  title: string
  description?: string
  category?: string
  yesLabel: string
  noLabel: string
  yesPrice: number
  noPrice: number
  endAt?: number
  displayStatus: string
  settlementStatus: string
}

export interface PredictionQuote {
  quoteId: string
  outcome: PredictionOutcome
  assetId: number
  assetSymbol: string
  stakeAmount: number
  feeAmount: number
  shares: number
  theoreticalPayout: number
  expiresAt: number
}

export interface PredictionOrder {
  id: number
  marketTitle: string
  outcome: string
  assetSymbol: string
  stakeAmount: number
  status: string
  payoutAmount: number
  createdAt: number
}

export async function fetchPredictionConfig(): Promise<PredictionAsset[]> {
  const response = await client.get<{ allowed_assets?: Array<Record<string, unknown>> }>(requestUrl('/prediction/config'))
  return (response.data.allowed_assets || []).map((asset) => ({
    assetId: asNumber(asset.asset_id),
    assetSymbol: String(asset.asset_symbol || '').toUpperCase(),
    maxPayoutAmount: asNumber(asset.max_payout_amount),
  }))
}

export async function fetchPredictionMarkets(limit = 50): Promise<PredictionMarket[]> {
  const response = await client.get<{ markets?: Array<Record<string, unknown>> }>(requestUrl('/prediction/markets'), { params: { limit } })
  return (response.data.markets || []).map((market) => ({
    id: asNumber(market.id),
    title: String(market.title || ''),
    description: optionalText(market.description),
    category: optionalText(market.category),
    yesLabel: String(market.outcome_yes_label || i18n.global.t('prediction.yes')),
    noLabel: String(market.outcome_no_label || i18n.global.t('prediction.no')),
    yesPrice: asNumber(market.yes_price),
    noPrice: asNumber(market.no_price),
    endAt: optionalTimestamp(market.end_at),
    displayStatus: String(market.display_status || ''),
    settlementStatus: String(market.settlement_status || ''),
  }))
}

export async function requestPredictionQuote(input: { marketId: number; outcome: PredictionOutcome; assetId: number; stakeAmount: number }): Promise<PredictionQuote> {
  const response = await client.post<Record<string, unknown>>(requestUrl('/prediction/quotes'), {
    market_id: input.marketId,
    outcome: input.outcome,
    asset_id: input.assetId,
    stake_amount: String(input.stakeAmount),
  })
  return {
    quoteId: String(response.data.quote_id || ''),
    outcome: String(response.data.outcome || 'yes') === 'no' ? 'no' : 'yes',
    assetId: asNumber(response.data.asset_id),
    assetSymbol: String(response.data.asset_symbol || '').toUpperCase(),
    stakeAmount: asNumber(response.data.stake_amount),
    feeAmount: asNumber(response.data.fee_amount),
    shares: asNumber(response.data.shares),
    theoreticalPayout: asNumber(response.data.theoretical_payout),
    expiresAt: normalizeTimestamp(response.data.expires_at),
  }
}

export async function confirmPredictionQuote(quoteId: string): Promise<void> {
  await client.post(requestUrl('/prediction/orders'), { quote_id: quoteId, idempotency_key: createIdempotencyKey('mobile-prediction') })
}

export async function fetchPredictionOrders(limit = 50): Promise<PredictionOrder[]> {
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/prediction/orders'), { params: { limit } })
  return (response.data.orders || []).map((order) => ({
    id: asNumber(order.id),
    marketTitle: String(order.market_title || ''),
    outcome: String(order.outcome || ''),
    assetSymbol: String(order.asset_symbol || '').toUpperCase(),
    stakeAmount: asNumber(order.stake_amount),
    status: String(order.status || ''),
    payoutAmount: asNumber(order.payout_amount),
    createdAt: normalizeTimestamp(order.created_at),
  }))
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function optionalTimestamp(value: unknown): number | undefined {
  const timestamp = normalizeTimestamp(value)
  return timestamp || undefined
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
