import { client, publicApiRequestConfig, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import {
  decimalCompare,
  decimalDivide,
  normalizeDecimalText,
  type DecimalText,
} from '@/core/decimal'
import {
  mapNewCoinDistribution,
  mapNewCoinProject,
  mapNewCoinPurchase,
  mapNewCoinSubscription,
  mapNewCoinUnlock,
  type NewCoinDistribution,
  type NewCoinProject,
  type NewCoinPurchase,
  type NewCoinSubscription,
  type NewCoinUnlock,
} from '@/core/newCoinModel'

export type {
  NewCoinDistribution,
  NewCoinProject,
  NewCoinPurchase,
  NewCoinSubscription,
  NewCoinUnlock,
} from '@/core/newCoinModel'

export async function fetchNewCoinProjects(limit = 50, options: ReferenceRequestOptions = {}): Promise<NewCoinProject[]> {
  const url = requestUrl('/new-coins')
  return referenceRequestRegistry.request(createReferenceRequestKey(url, { limit }), 30_000, async () => {
    const response = await client.get<{ projects?: Array<Record<string, unknown>> }>(
      url,
      publicApiRequestConfig({ params: { limit } }),
    )
    return (response.data.projects || []).map(mapNewCoinProject)
  }, options)
}

export async function fetchNewCoinProject(symbol: string): Promise<NewCoinProject> {
  const response = await client.get<Record<string, unknown>>(
    requestUrl(`/new-coins/${encodeURIComponent(symbol)}`),
    publicApiRequestConfig(),
  )
  return mapNewCoinProject(response.data)
}

export async function subscribeNewCoin(input: {
  symbol: string
  quoteAssetId: number
  quoteAmount: DecimalText
  issuePrice: DecimalText
}): Promise<void> {
  const quoteAmount = normalizeDecimalText(input.quoteAmount)
  const issuePrice = normalizeDecimalText(input.issuePrice)
  if (decimalCompare(issuePrice, normalizeDecimalText('0')) <= 0) {
    throw new RangeError('new-coin issue price must be positive')
  }
  await client.post(requestUrl(`/new-coins/${encodeURIComponent(input.symbol)}/subscriptions`), {
    quote_asset_id: input.quoteAssetId,
    quote_amount: quoteAmount,
    quantity: decimalDivide(quoteAmount, issuePrice, 18),
    idempotency_key: createIdempotencyKey('mobile-new-coin'),
  })
}

export async function fetchNewCoinSubscriptions(limit = 50): Promise<NewCoinSubscription[]> {
  const response = await client.get<{ subscriptions?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/subscriptions'), { params: { limit } })
  return (response.data.subscriptions || []).map(mapNewCoinSubscription)
}

export async function fetchNewCoinDistributions(limit = 50): Promise<NewCoinDistribution[]> {
  const response = await client.get<{ distributions?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/distributions'), { params: { limit } })
  return (response.data.distributions || []).map(mapNewCoinDistribution)
}

export async function fetchNewCoinPurchases(limit = 50): Promise<NewCoinPurchase[]> {
  const response = await client.get<{ purchases?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/purchases'), { params: { limit } })
  return (response.data.purchases || []).map(mapNewCoinPurchase)
}

export async function fetchNewCoinUnlocks(limit = 50): Promise<NewCoinUnlock[]> {
  const response = await client.get<{ unlocks?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/unlocks'), { params: { limit } })
  return (response.data.unlocks || []).map(mapNewCoinUnlock)
}

export async function createNewCoinPurchase(input: {
  symbol: string
  pairId: number
  price: DecimalText
  quantity: DecimalText
}): Promise<void> {
  await client.post(requestUrl(`/new-coins/${encodeURIComponent(input.symbol)}/purchase`), {
    pair_id: input.pairId,
    price: normalizeDecimalText(input.price),
    quantity: normalizeDecimalText(input.quantity),
    idempotency_key: createIdempotencyKey('mobile-new-coin-purchase'),
  })
}

export async function payNewCoinUnlockFee(input: {
  idempotencyKey: string
  paymentAssetId: number
  amount: DecimalText
}): Promise<void> {
  await client.post(requestUrl(`/new-coins/unlocks/${encodeURIComponent(input.idempotencyKey)}/pay-fee`), {
    payment_asset_id: input.paymentAssetId,
    amount: normalizeDecimalText(input.amount),
  })
}

export async function releaseNewCoinUnlock(idempotencyKey: string): Promise<void> {
  await client.post(requestUrl(`/new-coins/unlocks/${encodeURIComponent(idempotencyKey)}/release`), {})
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
