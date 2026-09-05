import request from './request'
import {
  backendApiUrl,
  mapNewCoinProjectsToPcActivityPage,
  mapPcNewCoinSubscriptionRequest,
  type BackendNewCoinProject,
  type BackendNewCoinProjectsResponse,
  type BackendWalletAccountsResponse,
} from './backendAdapters'

export interface IEOProject {
  id: number
  title: string
  titleEN?: string
  detail: string
  detailEN?: string
  smallImageUrl: string
  bannerImageUrl: string
  status: number
  step: number
  progress: number
  startTime: string
  endTime: string
  type: number
  totalSupply: number
  tradedAmount: number
  price: number
  priceScale: number
  unit: string
  acceptUnit: string
  acceptAssetId?: number
  amountScale: number
  maxLimitAmout: number
  minLimitAmout: number
  holdLimit: number
  holdUnit: string
  limitTimes: number
  miningPeriod: number
  miningDays: number
  miningUnit: string
  lockedPeriod: number
  lockedDays: number
  releaseType: number
  releasePercent: number
  releaseAmount: number
  content: string
  contentEN?: string
}

export interface ActivityPageResponse {
  content: IEOProject[]
  page: {
    size: number
    number: number
    totalElements: number
    totalPages: number
  }
}

export async function fetchActivityList(pageNo: number = 1, pageSize: number = 10, step: number = -1): Promise<{ data: any }> {
  const response = await request.instance.get<BackendNewCoinProjectsResponse>(backendApiUrl('/new-coins'), {
    params: { limit: pageSize },
  })
  return { data: mapNewCoinProjectsToPcActivityPage(response.data, { pageNo, pageSize, step }) }
}

export async function fetchActivityDetail(symbolOrId: number | string): Promise<{ data: any }> {
  const symbol = await resolveProjectSymbol(symbolOrId)
  const response = await request.instance.get<BackendNewCoinProject>(backendApiUrl(`/new-coins/${encodeURIComponent(symbol)}`))
  return { data: mapNewCoinProjectsToPcActivityPage({ projects: [response.data] }, { pageNo: 1, pageSize: 1 }).data.content[0] }
}

export async function attendActivity(params: { id?: number; symbol?: string; unit?: string; amount: number; price?: number; quoteAssetId?: number }): Promise<{ data: any }> {
  const symbol = params.symbol || params.unit || await resolveProjectSymbol(params.id)
  const quoteAssetId = params.quoteAssetId ?? await resolveAssetId('USDT')
  const price = params.price ?? await resolveProjectPrice(symbol)
  const response = await request.instance.post(backendApiUrl(`/new-coins/${encodeURIComponent(symbol)}/subscriptions`), mapPcNewCoinSubscriptionRequest({
    quoteAssetId,
    amount: params.amount,
    price,
  }, createNewCoinIdempotencyKey()))

  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export interface NewCoinSubscriptionRecord {
  id: number
  projectId: number
  quoteAsset: number
  quoteAmount: string
  requestedQuantity: string
  allocatedQuantity: string
  status: string
  createdAt: number
  settlementMode?: string
  frozenQuoteAmount?: string
  settledQuoteAmount?: string
  refundedQuoteAmount?: string
}

export interface NewCoinDistributionRecord {
  id: number
  projectId: number
  subscriptionId: number | null
  assetId: number
  quantity: string
  lockPositionId: number | null
  status: string
  createdAt: number
}

export interface NewCoinPurchaseRecord {
  id: number
  projectId: number
  pairId: number
  baseAsset: number
  quoteAsset: number
  price: string
  quantity: string
  quoteAmount: string
  lockPositionId: number | null
  status: string
  createdAt: number
}

export interface NewCoinUnlockRecord {
  id: number
  assetId: number
  lockPositionId: number
  unlockQuantity: string
  unlockPrice: string | null
  unlockFeeEnabled: boolean
  unlockFeeAsset: number | null
  unlockFeeAmount: string | null
  feePaidStatus: string
  status: string
  idempotencyKey: string
  createdAt: number
}

export async function fetchNewCoinSubscriptions(limit = 50): Promise<NewCoinSubscriptionRecord[]> {
  const response = await request.instance.get<{ subscriptions: any[] }>(backendApiUrl('/new-coins/subscriptions'), { params: { limit } })
  return (response.data.subscriptions ?? []).map((item) => ({
    id: Number(item.id),
    projectId: Number(item.project_id),
    quoteAsset: Number(item.quote_asset),
    settlementMode: item.settlement_mode == null ? undefined : String(item.settlement_mode),
    frozenQuoteAmount: item.frozen_quote_amount == null ? undefined : String(item.frozen_quote_amount),
    settledQuoteAmount: item.settled_quote_amount == null ? undefined : String(item.settled_quote_amount),
    refundedQuoteAmount: item.refunded_quote_amount == null ? undefined : String(item.refunded_quote_amount),
    quoteAmount: String(item.quote_amount ?? '0'),
    requestedQuantity: String(item.requested_quantity ?? '0'),
    allocatedQuantity: String(item.allocated_quantity ?? '0'),
    status: String(item.status ?? ''),
    createdAt: Number(item.created_at ?? 0),
  }))
}

export async function fetchNewCoinDistributions(limit = 50): Promise<NewCoinDistributionRecord[]> {
  const response = await request.instance.get<{ distributions: any[] }>(backendApiUrl('/new-coins/distributions'), { params: { limit } })
  return (response.data.distributions ?? []).map((item) => ({
    id: Number(item.id),
    projectId: Number(item.project_id),
    subscriptionId: item.subscription_id == null ? null : Number(item.subscription_id),
    assetId: Number(item.asset_id),
    quantity: String(item.quantity ?? '0'),
    lockPositionId: item.lock_position_id == null ? null : Number(item.lock_position_id),
    status: String(item.status ?? ''),
    createdAt: Number(item.created_at ?? 0),
  }))
}

export async function fetchNewCoinPurchases(limit = 50): Promise<NewCoinPurchaseRecord[]> {
  const response = await request.instance.get<{ purchases: any[] }>(backendApiUrl('/new-coins/purchases'), { params: { limit } })
  return (response.data.purchases ?? []).map((item) => ({
    id: Number(item.id),
    projectId: Number(item.project_id),
    pairId: Number(item.pair_id),
    baseAsset: Number(item.base_asset),
    quoteAsset: Number(item.quote_asset),
    price: String(item.price ?? '0'),
    quantity: String(item.quantity ?? '0'),
    quoteAmount: String(item.quote_amount ?? '0'),
    lockPositionId: item.lock_position_id == null ? null : Number(item.lock_position_id),
    status: String(item.status ?? ''),
    createdAt: Number(item.created_at ?? 0),
  }))
}

export async function fetchNewCoinUnlocks(limit = 50): Promise<NewCoinUnlockRecord[]> {
  const response = await request.instance.get<{ unlocks: any[] }>(backendApiUrl('/new-coins/unlocks'), { params: { limit } })
  return (response.data.unlocks ?? []).map((item) => ({
    id: Number(item.id),
    assetId: Number(item.asset_id),
    lockPositionId: Number(item.lock_position_id),
    unlockQuantity: String(item.unlock_quantity ?? '0'),
    unlockPrice: item.unlock_price == null ? null : String(item.unlock_price),
    unlockFeeEnabled: Boolean(item.unlock_fee_enabled),
    unlockFeeAsset: item.unlock_fee_asset == null ? null : Number(item.unlock_fee_asset),
    unlockFeeAmount: item.unlock_fee_amount == null ? null : String(item.unlock_fee_amount),
    feePaidStatus: String(item.fee_paid_status ?? ''),
    status: String(item.status ?? ''),
    idempotencyKey: String(item.idempotency_key ?? ''),
    createdAt: Number(item.created_at ?? 0),
  }))
}

export async function payNewCoinUnlockFee(idempotencyKey: string, paymentAssetId: number, amount: string): Promise<void> {
  await request.instance.post(backendApiUrl(`/new-coins/unlocks/${encodeURIComponent(idempotencyKey)}/pay-fee`), {
    payment_asset_id: paymentAssetId,
    amount,
  })
}

export async function releaseNewCoinUnlock(idempotencyKey: string): Promise<void> {
  await request.instance.post(backendApiUrl(`/new-coins/unlocks/${encodeURIComponent(idempotencyKey)}/release`), {})
}

/// 后端记录只带资产 ID，用钱包账户表补出符号用于展示。
export async function fetchAssetSymbolMap(): Promise<Record<number, string>> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  return Object.fromEntries(response.data.accounts.map((account) => [account.asset_id, account.symbol]))
}

async function resolveProjectSymbol(symbolOrId?: number | string): Promise<string> {
  if (typeof symbolOrId === 'string' && symbolOrId.trim()) return symbolOrId
  const response = await request.instance.get<BackendNewCoinProjectsResponse>(backendApiUrl('/new-coins'))
  const project = response.data.projects.find((item) => item.id === symbolOrId)
  if (!project) throw new Error(`New coin project unavailable: ${symbolOrId}`)
  return project.symbol
}

async function resolveProjectPrice(symbol: string): Promise<number> {
  const response = await request.instance.get<BackendNewCoinProject>(backendApiUrl(`/new-coins/${encodeURIComponent(symbol)}`))
  return Number(response.data.issue_price) || 0
}

async function resolveAssetId(symbol: string): Promise<number> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  const account = response.data.accounts.find((item) => item.symbol.toUpperCase() === symbol.toUpperCase())
  if (!account) throw new Error(`Wallet asset unavailable: ${symbol}`)
  return account.asset_id
}

function createNewCoinIdempotencyKey(): string {
  return `pc-new-coin-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
