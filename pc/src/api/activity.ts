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
