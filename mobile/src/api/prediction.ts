import { client, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { requiredId, requiredSafeInteger, normalizeTimestamp } from '@/core/numeric'
import { i18n } from '@/i18n'
import { normalizeDecimalText, requiredDecimalText, type DecimalText } from '@/core/decimal'

export type PredictionOutcome = 'yes' | 'no'

export interface PredictionAsset {
  assetId: number
  assetSymbol: string
  maxPayoutAmount: DecimalText
}

export interface PredictionMarket {
  id: number
  title: string
  description?: string
  category?: string
  yesLabel: string
  noLabel: string
  yesPrice: DecimalText
  noPrice: DecimalText
  endAt?: number
  displayStatus: string
  settlementStatus: string
}

export interface PredictionQuote {
  quoteId: string
  outcome: PredictionOutcome
  assetId: number
  assetSymbol: string
  stakeAmount: DecimalText
  feeAmount: DecimalText
  shares: DecimalText
  theoreticalPayout: DecimalText
  expiresAt: number
}

export interface PredictionOrder {
  id: number
  orderNo: string
  marketTitle: string
  outcome: string
  assetSymbol: string
  stakeAmount: DecimalText
  status: string
  result?: string
  payoutAmount: DecimalText
  refundAmount: DecimalText
  createdAt: number
}

export async function fetchPredictionConfig(options: ReferenceRequestOptions = {}): Promise<PredictionAsset[]> {
  const url = requestUrl('/prediction/config')
  return referenceRequestRegistry.request(createReferenceRequestKey(url), 2 * 60_000, async () => {
    const response = await client.get<{ allowed_assets?: Array<Record<string, unknown>> }>(url)
    return (response.data.allowed_assets || []).map((asset) => ({
      assetId: requiredId(asset.asset_id),
      assetSymbol: String(asset.asset_symbol || '').toUpperCase(),
      maxPayoutAmount: predictionDecimal(asset.max_payout_amount, 'max_payout_amount'),
    }))
  }, options)
}

export async function fetchPredictionMarkets(limit = 50): Promise<PredictionMarket[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const response = await client.get<{ markets?: Array<Record<string, unknown>> }>(requestUrl('/prediction/markets'), { params: { limit } })
  return (response.data.markets || []).map((market) => ({
    id: requiredId(market.id),
    title: String(market.title || ''),
    description: optionalText(market.description),
    category: optionalText(market.category),
    yesLabel: String(market.outcome_yes_label || i18n.global.t('prediction.yes')),
    noLabel: String(market.outcome_no_label || i18n.global.t('prediction.no')),
    yesPrice: predictionDecimal(market.yes_price, 'yes_price'),
    noPrice: predictionDecimal(market.no_price, 'no_price'),
    endAt: optionalTimestamp(market.end_at),
    displayStatus: String(market.display_status || ''),
    settlementStatus: String(market.settlement_status || ''),
  }))
}

export async function requestPredictionQuote(input: {
  marketId: number
  outcome: PredictionOutcome
  assetId: number
  stakeAmount: DecimalText
}): Promise<PredictionQuote> {
  const response = await client.post<Record<string, unknown>>(requestUrl('/prediction/quotes'), {
    market_id: requiredId(input.marketId),
    outcome: input.outcome,
    asset_id: requiredId(input.assetId),
    stake_amount: normalizeDecimalText(input.stakeAmount),
  })
  return {
    quoteId: String(response.data.quote_id || ''),
    outcome: predictionOutcome(response.data.outcome),
    assetId: requiredId(response.data.asset_id),
    assetSymbol: String(response.data.asset_symbol || '').toUpperCase(),
    stakeAmount: predictionDecimal(response.data.stake_amount, 'stake_amount'),
    feeAmount: predictionDecimal(response.data.fee_amount, 'fee_amount'),
    shares: predictionDecimal(response.data.shares, 'shares'),
    theoreticalPayout: predictionDecimal(response.data.theoretical_payout, 'theoretical_payout'),
    expiresAt: normalizeTimestamp(response.data.expires_at),
  }
}

export async function confirmPredictionQuote(quoteId: string): Promise<PredictionOrder> {
  const response = await client.post<{ order?: Record<string, unknown> }>(requestUrl('/prediction/orders'), {
    quote_id: quoteId,
    idempotency_key: createIdempotencyKey('mobile-prediction'),
  })
  if (!response.data.order) throw new Error('Prediction order response is missing order data')
  return mapPredictionOrder(response.data.order)
}

export async function fetchPredictionOrders(limit = 50): Promise<PredictionOrder[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/prediction/orders'), { params: { limit } })
  return (response.data.orders || []).map(mapPredictionOrder)
}

function mapPredictionOrder(order: Record<string, unknown>): PredictionOrder {
  return {
    id: requiredId(order.id),
    orderNo: String(order.order_no || ''),
    marketTitle: String(order.market_title || ''),
    outcome: String(order.outcome || ''),
    assetSymbol: String(order.asset_symbol || '').toUpperCase(),
    stakeAmount: predictionDecimal(order.stake_amount, 'stake_amount'),
    status: String(order.status || ''),
    result: optionalText(order.result),
    payoutAmount: predictionDecimal(order.payout_amount, 'payout_amount'),
    refundAmount: predictionDecimal(order.refund_amount, 'refund_amount'),
    createdAt: normalizeTimestamp(order.created_at),
  }
}

function predictionOutcome(value: unknown): PredictionOutcome {
  const outcome = String(value || '').trim().toLowerCase()
  if (outcome === 'yes' || outcome === 'no') return outcome
  throw new Error('Prediction quote response contains an invalid outcome')
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function optionalTimestamp(value: unknown): number | undefined {
  const timestamp = normalizeTimestamp(value)
  return timestamp || undefined
}

function predictionDecimal(value: unknown, field: string): DecimalText {
  return requiredDecimalText(value, field, 'prediction', { allowNegative: false, maxIntegerDigits: 20, maxScale: 18 })
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
