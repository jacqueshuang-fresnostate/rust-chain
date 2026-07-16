import request from './request'
import {
  backendApiUrl,
  mapEarnProductsToPcFinanceList,
  mapEarnSubscriptionsToPcFinanceCount,
  mapEarnSubscriptionsToPcFinancePage,
  mapEarnSubscriptionsToPcFinanceStatistic,
  type BackendEarnProductsResponse,
  type BackendEarnSubscriptionsResponse,
} from './backendAdapters'

export interface FinanceProduct {
  id: number
  status: number
  step: number
  acceptUnit: string
  maxLimitAmount: number
  minLimitAmount: number
  minDaysProfit: number
  cycle: number
  iconImageUrl: string
}

export interface FinanceStatistic {
  id: number
  memberId: number
  coinSymbol: string
  num: number
  earnNum: number
}

export enum AiFinanceOrderStatus {
  OPEN = 0,
  CLOSE = 1,
}

export interface AiFinanceOrder {
  id: number
  orderNo: string
  memberId: number
  financeId: number
  coinSymbol: string
  num: number
  status: AiFinanceOrderStatus
  earnNum: number
  hourCursor: number
  cycle: number
  breachFee: number
  minDaysProfit: number
  maxDaysProfit: number
  createTime: number | string
  updateTime: number | string
}

export interface ApiResponse<T> {
  code: number
  message: string
  data: T
}

export async function fetchFinanceList(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendEarnProductsResponse>(backendApiUrl('/earn/products'))
  return { data: mapEarnProductsToPcFinanceList(response.data) }
}

export async function fetchFinanceStatistic(symbol: string = 'USDT'): Promise<{ data: any }> {
  const response = await request.instance.get<BackendEarnSubscriptionsResponse>(backendApiUrl('/earn/subscriptions'))
  return { data: mapEarnSubscriptionsToPcFinanceStatistic(response.data, symbol.split('-')[0]) }
}

export async function fetchFinanceCount(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendEarnSubscriptionsResponse>(backendApiUrl('/earn/subscriptions'))
  return { data: mapEarnSubscriptionsToPcFinanceCount(response.data) }
}

export async function investFinance(params: { id: number; amount: number }): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl('/earn/subscriptions'), {
    product_id: params.id,
    amount: String(params.amount),
    idempotency_key: createEarnIdempotencyKey(),
  })
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export async function fetchFinanceHistory(
  status?: AiFinanceOrderStatus,
  pageNo: number = 1,
  pageSize: number = 10,
  symbol?: string,
): Promise<{ data: any }> {
  const response = await request.instance.get<BackendEarnSubscriptionsResponse>(backendApiUrl('/earn/subscriptions'))
  const page = mapEarnSubscriptionsToPcFinancePage(response.data, { pageNo, pageSize, status })
  if (!symbol) return { data: page }

  return {
    data: {
      ...page,
      data: {
        ...page.data,
        content: page.data.content.filter((item) => item.coinSymbol === symbol.split('-')[0]),
      },
    },
  }
}

export async function closeFinanceOrder(id: number): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl(`/earn/subscriptions/${id}/redeem`), {})
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

function createEarnIdempotencyKey(): string {
  return `pc-earn-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
