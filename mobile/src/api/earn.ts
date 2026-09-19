import { client, readAuthSessionSnapshot, requestUrl } from './client'
import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { requiredId, requiredSafeInteger, normalizeTimestamp } from '@/core/numeric'
import { i18n } from '@/i18n'
import { requiredDecimalText, normalizeDecimalText, type DecimalText } from '@/core/decimal'

const subscriptionKeys = new RetryStableIdempotencyKeys('mobile-earn', undefined, () => globalThis.sessionStorage)

export interface EarnProduct {
  id: number
  assetId: number
  assetSymbol: string
  name: string
  category: string
  termDays: number
  aprRate: DecimalText
  redemptionFeeRate?: DecimalText
  maturityProfitFeeRate?: DecimalText
  earlyRedeemFeeBasis?: string
  earlyRedeemFeeRate?: DecimalText
  minSubscribe: DecimalText
  maxSubscribe?: DecimalText
  minSubscribeText?: DecimalText
  maxSubscribeText?: DecimalText
  status: string
}

export interface EarnSubscription {
  id: number
  productId: number
  assetId: number
  amount: DecimalText
  aprRate: DecimalText
  termDays: number
  status: string
  subscribedAt: number
  maturesAt: number
}

export async function fetchEarnProducts(limit = 50, options: ReferenceRequestOptions = {}): Promise<EarnProduct[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const url = requestUrl('/earn/products')
  return referenceRequestRegistry.request(createReferenceRequestKey(url, {
    limit,
    locale: i18n.global.locale.value,
  }), 60_000, async () => {
    const response = await client.get<{ products?: Array<Record<string, unknown>> }>(url, { params: { limit } })
    return (response.data.products || []).map((product) => ({
      id: requiredId(product.id),
      assetId: requiredId(product.asset_id),
      assetSymbol: String(product.asset_symbol || '').toUpperCase(),
      name: String(product.name || ''),
      category: String(product.category_name || product.category || i18n.global.t('earn.defaultCategory')),
      termDays: requiredSafeInteger(product.term_days, 'term_days'),
      aprRate: productDecimal(product.apr_rate),
      redemptionFeeRate: optionalDecimal(product.redemption_fee_rate),
      maturityProfitFeeRate: optionalDecimal(product.maturity_profit_fee_rate),
      earlyRedeemFeeBasis: optionalText(product.early_redeem_fee_basis),
      earlyRedeemFeeRate: optionalDecimal(product.early_redeem_fee_rate),
      minSubscribe: productDecimal(product.min_subscribe),
      maxSubscribe: optionalDecimal(product.max_subscribe),
      minSubscribeText: productDecimal(product.min_subscribe),
      maxSubscribeText: product.max_subscribe === null || product.max_subscribe === undefined
        ? undefined
        : productDecimal(product.max_subscribe),
      status: String(product.status || ''),
    }))
  }, options)
}

export async function fetchEarnSubscriptions(limit = 50): Promise<EarnSubscription[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const response = await client.get<{ subscriptions?: Array<Record<string, unknown>> }>(requestUrl('/earn/subscriptions'), { params: { limit } })
  return (response.data.subscriptions || []).map(mapSubscription)
}

export async function subscribeEarnProduct(productId: number, amount: DecimalText): Promise<void> {
  const scope = readAuthSessionSnapshot().scope
  if (!scope) throw new Error('authenticated session is required')
  const payload = {
    product_id: requiredId(productId),
    amount: normalizeDecimalText(amount),
  }
  const intent = canonicalRequestIntent({ ...payload, scope })
  const key = subscriptionKeys.acquire(intent)
  await client.post(requestUrl('/earn/subscriptions'), {
    ...payload,
    idempotency_key: key,
  })
  subscriptionKeys.complete(intent, key)
}

export async function redeemEarnSubscription(subscriptionId: number): Promise<void> {
  requiredId(subscriptionId)
  await client.post(requestUrl(`/earn/subscriptions/${subscriptionId}/redeem`), {})
}

function mapSubscription(subscription: Record<string, unknown>): EarnSubscription {
  return {
    id: requiredId(subscription.id),
    productId: requiredId(subscription.product_id),
    assetId: requiredId(subscription.asset_id),
    amount: productDecimal(subscription.amount),
    aprRate: productDecimal(subscription.apr_rate),
    termDays: requiredSafeInteger(subscription.term_days, 'term_days'),
    status: String(subscription.status || ''),
    subscribedAt: normalizeTimestamp(subscription.subscribed_at),
    maturesAt: normalizeTimestamp(subscription.matures_at),
  }
}

function optionalDecimal(value: unknown): DecimalText | undefined {
  return value === null || value === undefined ? undefined : productDecimal(value)
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function productDecimal(value: unknown): DecimalText {
  return requiredDecimalText(value, 'decimal', 'earn', { allowNegative: false, maxIntegerDigits: 20, maxScale: 18 })
}
