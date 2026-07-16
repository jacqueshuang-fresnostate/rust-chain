import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'
import { i18n } from '@/i18n'

export interface EarnProduct {
  id: number
  assetId: number
  assetSymbol: string
  name: string
  category: string
  termDays: number
  aprRate: number
  minSubscribe: number
  maxSubscribe?: number
  status: string
}

export interface EarnSubscription {
  id: number
  productId: number
  assetId: number
  amount: number
  aprRate: number
  termDays: number
  status: string
  subscribedAt: number
  maturesAt: number
}

export async function fetchEarnProducts(limit = 50): Promise<EarnProduct[]> {
  const response = await client.get<{ products?: Array<Record<string, unknown>> }>(requestUrl('/earn/products'), { params: { limit } })
  return (response.data.products || []).map((product) => ({
    id: asNumber(product.id),
    assetId: asNumber(product.asset_id),
    assetSymbol: String(product.asset_symbol || '').toUpperCase(),
    name: String(product.name || ''),
    category: String(product.category_name || product.category || i18n.global.t('earn.defaultCategory')),
    termDays: asNumber(product.term_days),
    aprRate: asNumber(product.apr_rate),
    minSubscribe: asNumber(product.min_subscribe),
    maxSubscribe: product.max_subscribe === null || product.max_subscribe === undefined ? undefined : asNumber(product.max_subscribe),
    status: String(product.status || ''),
  }))
}

export async function fetchEarnSubscriptions(limit = 50): Promise<EarnSubscription[]> {
  const response = await client.get<{ subscriptions?: Array<Record<string, unknown>> }>(requestUrl('/earn/subscriptions'), { params: { limit } })
  return (response.data.subscriptions || []).map(mapSubscription)
}

export async function subscribeEarnProduct(productId: number, amount: number): Promise<void> {
  await client.post(requestUrl('/earn/subscriptions'), {
    product_id: productId,
    amount: String(amount),
    idempotency_key: createIdempotencyKey('mobile-earn'),
  })
}

export async function redeemEarnSubscription(subscriptionId: number): Promise<void> {
  await client.post(requestUrl(`/earn/subscriptions/${subscriptionId}/redeem`), {})
}

function mapSubscription(subscription: Record<string, unknown>): EarnSubscription {
  return {
    id: asNumber(subscription.id),
    productId: asNumber(subscription.product_id),
    assetId: asNumber(subscription.asset_id),
    amount: asNumber(subscription.amount),
    aprRate: asNumber(subscription.apr_rate),
    termDays: asNumber(subscription.term_days),
    status: String(subscription.status || ''),
    subscribedAt: normalizeTimestamp(subscription.subscribed_at),
    maturesAt: normalizeTimestamp(subscription.matures_at),
  }
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
