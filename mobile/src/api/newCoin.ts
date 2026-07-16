import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'

export interface NewCoinProject {
  id: number
  assetId: number
  symbol: string
  lifecycleStatus: string
  totalSupply: number
  issuePrice: number
  listedAt?: number
  unlockType: string
  fixedUnlockAt?: number
  relativeUnlockSeconds?: number
  unlockFeeEnabled: boolean
  unlockFeeRate?: number
  unlockFeeBasis?: string
  unlockFeeAssetId?: number
  postListingPurchaseEnabled: boolean
  postListingPairId?: number
  status: string
}

export interface NewCoinSubscription {
  id: number
  projectId: number
  quoteAmount: number
  requestedQuantity: number
  allocatedQuantity: number
  status: string
  createdAt: number
}

export interface NewCoinDistribution {
  id: number
  projectId: number
  subscriptionId?: number
  assetId: number
  quantity: number
  lockPositionId?: number
  status: string
  idempotencyKey: string
  createdAt: number
}

export interface NewCoinPurchase {
  id: number
  projectId: number
  pairId: number
  baseAssetId: number
  quoteAssetId: number
  price: number
  quantity: number
  quoteAmount: number
  lockPositionId?: number
  status: string
  idempotencyKey: string
  createdAt: number
}

export interface NewCoinUnlock {
  id: number
  assetId: number
  lockPositionId: number
  unlockQuantity: number
  unlockPrice?: number
  unlockFeeEnabled: boolean
  unlockFeeRate?: number
  unlockFeeBasis?: string
  unlockFeeAssetId?: number
  unlockFeeAmount?: number
  feePaidStatus: string
  status: string
  idempotencyKey: string
  createdAt: number
}

export async function fetchNewCoinProjects(limit = 50): Promise<NewCoinProject[]> {
  const response = await client.get<{ projects?: Array<Record<string, unknown>> }>(requestUrl('/new-coins'), { params: { limit } })
  return (response.data.projects || []).map(mapProject)
}

export async function fetchNewCoinProject(symbol: string): Promise<NewCoinProject> {
  const response = await client.get<Record<string, unknown>>(requestUrl(`/new-coins/${encodeURIComponent(symbol)}`))
  return mapProject(response.data)
}

export async function subscribeNewCoin(input: { symbol: string; quoteAssetId: number; quoteAmount: number; issuePrice: number }): Promise<void> {
  await client.post(requestUrl(`/new-coins/${encodeURIComponent(input.symbol)}/subscriptions`), {
    quote_asset_id: input.quoteAssetId,
    quote_amount: String(input.quoteAmount),
    quantity: String(input.issuePrice > 0 ? input.quoteAmount / input.issuePrice : 0),
    idempotency_key: createIdempotencyKey('mobile-new-coin'),
  })
}

export async function fetchNewCoinSubscriptions(limit = 50): Promise<NewCoinSubscription[]> {
  const response = await client.get<{ subscriptions?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/subscriptions'), { params: { limit } })
  return (response.data.subscriptions || []).map((subscription) => ({
    id: asNumber(subscription.id),
    projectId: asNumber(subscription.project_id),
    quoteAmount: asNumber(subscription.quote_amount),
    requestedQuantity: asNumber(subscription.requested_quantity),
    allocatedQuantity: asNumber(subscription.allocated_quantity),
    status: String(subscription.status || ''),
    createdAt: normalizeTimestamp(subscription.created_at),
  }))
}

export async function fetchNewCoinDistributions(limit = 50): Promise<NewCoinDistribution[]> {
  const response = await client.get<{ distributions?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/distributions'), { params: { limit } })
  return (response.data.distributions || []).map((distribution) => ({
    id: asNumber(distribution.id),
    projectId: asNumber(distribution.project_id),
    subscriptionId: optionalNumber(distribution.subscription_id),
    assetId: asNumber(distribution.asset_id),
    quantity: asNumber(distribution.quantity),
    lockPositionId: optionalNumber(distribution.lock_position_id),
    status: String(distribution.status || ''),
    idempotencyKey: String(distribution.idempotency_key || ''),
    createdAt: normalizeTimestamp(distribution.created_at),
  }))
}

export async function fetchNewCoinPurchases(limit = 50): Promise<NewCoinPurchase[]> {
  const response = await client.get<{ purchases?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/purchases'), { params: { limit } })
  return (response.data.purchases || []).map((purchase) => ({
    id: asNumber(purchase.id),
    projectId: asNumber(purchase.project_id),
    pairId: asNumber(purchase.pair_id),
    baseAssetId: asNumber(purchase.base_asset),
    quoteAssetId: asNumber(purchase.quote_asset),
    price: asNumber(purchase.price),
    quantity: asNumber(purchase.quantity),
    quoteAmount: asNumber(purchase.quote_amount),
    lockPositionId: optionalNumber(purchase.lock_position_id),
    status: String(purchase.status || ''),
    idempotencyKey: String(purchase.idempotency_key || ''),
    createdAt: normalizeTimestamp(purchase.created_at),
  }))
}

export async function fetchNewCoinUnlocks(limit = 50): Promise<NewCoinUnlock[]> {
  const response = await client.get<{ unlocks?: Array<Record<string, unknown>> }>(requestUrl('/new-coins/unlocks'), { params: { limit } })
  return (response.data.unlocks || []).map((unlock) => ({
    id: asNumber(unlock.id),
    assetId: asNumber(unlock.asset_id),
    lockPositionId: asNumber(unlock.lock_position_id),
    unlockQuantity: asNumber(unlock.unlock_quantity),
    unlockPrice: optionalNumber(unlock.unlock_price),
    unlockFeeEnabled: Boolean(unlock.unlock_fee_enabled),
    unlockFeeRate: optionalNumber(unlock.unlock_fee_rate),
    unlockFeeBasis: optionalText(unlock.unlock_fee_basis),
    unlockFeeAssetId: optionalNumber(unlock.unlock_fee_asset),
    unlockFeeAmount: optionalNumber(unlock.unlock_fee_amount),
    feePaidStatus: String(unlock.fee_paid_status || ''),
    status: String(unlock.status || ''),
    idempotencyKey: String(unlock.idempotency_key || ''),
    createdAt: normalizeTimestamp(unlock.created_at),
  }))
}

export async function createNewCoinPurchase(input: { symbol: string; pairId: number; price: number; quantity: number }): Promise<void> {
  await client.post(requestUrl(`/new-coins/${encodeURIComponent(input.symbol)}/purchase`), {
    pair_id: input.pairId,
    price: String(input.price),
    quantity: String(input.quantity),
    idempotency_key: createIdempotencyKey('mobile-new-coin-purchase'),
  })
}

export async function payNewCoinUnlockFee(input: { idempotencyKey: string; paymentAssetId: number; amount: number }): Promise<void> {
  await client.post(requestUrl(`/new-coins/unlocks/${encodeURIComponent(input.idempotencyKey)}/pay-fee`), {
    payment_asset_id: input.paymentAssetId,
    amount: String(input.amount),
  })
}

export async function releaseNewCoinUnlock(idempotencyKey: string): Promise<void> {
  await client.post(requestUrl(`/new-coins/unlocks/${encodeURIComponent(idempotencyKey)}/release`), {})
}

function mapProject(project: Record<string, unknown>): NewCoinProject {
  return {
    id: asNumber(project.id),
    assetId: asNumber(project.asset_id),
    symbol: String(project.symbol || '').toUpperCase(),
    lifecycleStatus: String(project.lifecycle_status || ''),
    totalSupply: asNumber(project.total_supply),
    issuePrice: asNumber(project.issue_price),
    listedAt: optionalTimestamp(project.listed_at),
    unlockType: String(project.unlock_type || ''),
    fixedUnlockAt: optionalTimestamp(project.fixed_unlock_at),
    relativeUnlockSeconds: optionalNumber(project.relative_unlock_seconds),
    unlockFeeEnabled: Boolean(project.unlock_fee_enabled),
    unlockFeeRate: optionalNumber(project.unlock_fee_rate),
    unlockFeeBasis: optionalText(project.unlock_fee_basis),
    unlockFeeAssetId: optionalNumber(project.unlock_fee_asset),
    postListingPurchaseEnabled: Boolean(project.post_listing_purchase_enabled),
    postListingPairId: optionalNumber(project.post_listing_pair_id),
    status: String(project.status || ''),
  }
}

function optionalTimestamp(value: unknown): number | undefined {
  const timestamp = normalizeTimestamp(value)
  return timestamp || undefined
}

function optionalNumber(value: unknown): number | undefined {
  const number = asNumber(value)
  return Number.isFinite(number) && number > 0 ? number : undefined
}

function optionalText(value: unknown): string | undefined {
  const text = String(value || '').trim()
  return text || undefined
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
