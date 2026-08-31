import request from './request'
import {
  backendApiUrl,
  mapPcSpotOrderRequest,
  mapSpotOrdersToPcPage,
  mapWalletAccountsToMemberWallets,
  mapWalletAccountsToTradeWallets,
  type BackendSpotOrder,
  type BackendSpotOrdersResponse,
  type BackendWalletAccountsResponse,
} from './backendAdapters'
import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency'

const spotOrderIdempotencyKeys = new RetryStableIdempotencyKeys('pc-spot')

// Order Types: LIMIT_PRICE, MARKET_PRICE, STOP_LIMIT
export type OrderType = 'LIMIT_PRICE' | 'MARKET_PRICE' | 'STOP_LIMIT'
export type OrderDirection = 'BUY' | 'SELL'

export interface OrderParams {
  symbol: string
  price?: number
  triggerPrice?: number
  amount: number // For LIMIT/MARKET SELL, and MARKET BUY (USDT amount)
  direction: OrderDirection
  type: OrderType
  useDiscount?: number
}

/**
 * Place a new order
 */
export async function addOrder(params: OrderParams): Promise<{ data: any }> {
  const intentPayload = mapPcSpotOrderRequest(params, '')
  const { idempotency_key: _ignoredKey, ...businessIntent } = intentPayload
  const intent = canonicalRequestIntent(businessIntent)
  const idempotencyKey = spotOrderIdempotencyKeys.acquire(intent)
  const response = await request.instance.post<BackendSpotOrder>(backendApiUrl('/spot/orders'), {
    ...intentPayload,
    idempotency_key: idempotencyKey,
  })
  spotOrderIdempotencyKeys.complete(intent, idempotencyKey)
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

/**
 * Cancel an order
 */
export async function cancelOrder(orderId: string): Promise<{ data: any }> {
  const response = await request.instance.delete(backendApiUrl(`/spot/orders/${orderId}`))
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

/**
 * Cancel every open order, optionally scoped to one pair
 */
export async function cancelAllOrders(pairId?: string): Promise<{ cancelled: number; failed: number }> {
  const response = await request.instance.delete<{ orders?: unknown[]; failures?: unknown[] }>(
    backendApiUrl('/spot/orders'),
    { params: pairId ? { pair_id: pairId } : undefined },
  )
  return {
    cancelled: response.data.orders?.length ?? 0,
    failed: response.data.failures?.length ?? 0,
  }
}

/**
 * Fetch Current Open Orders
 */
export async function fetchCurrentOrders(symbol: string, pageNo: number = 0, pageSize: number = 10): Promise<{ data: any }> {
  const [pending, open, partiallyFilled] = await Promise.all([
    fetchOrdersByStatus(symbol, 'pending', pageSize),
    fetchOrdersByStatus(symbol, 'open', pageSize),
    fetchOrdersByStatus(symbol, 'partially_filled', pageSize),
  ])
  return {
    data: mapSpotOrdersToPcPage(
      { orders: [...pending, ...open, ...partiallyFilled] },
      { pageNo, pageSize },
    ).data,
  }
}

/**
 * Fetch Order History
 */
export async function fetchHistoryOrders(symbol: string, pageNo: number = 0, pageSize: number = 10): Promise<{ data: any }> {
  const [filled, cancelled, rejected] = await Promise.all([
    fetchOrdersByStatus(symbol, 'filled', pageSize),
    fetchOrdersByStatus(symbol, 'cancelled', pageSize),
    fetchOrdersByStatus(symbol, 'rejected', pageSize),
  ])
  return {
    data: mapSpotOrdersToPcPage(
      { orders: [...filled, ...cancelled, ...rejected] },
      { pageNo, pageSize },
    ).data,
  }
}

/**
 * Fetch User Wallet for a specific symbol (Base + Quote)
 * e.g., for BTC/USDT, fetches BTC and USDT wallets
 */
export async function fetchWallet(symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  return { data: mapWalletAccountsToTradeWallets(response.data, symbol) }
}

/**
 * Fetch All Assets
 */
export async function fetchAssets(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  return { data: mapWalletAccountsToMemberWallets(response.data) }
}

async function fetchOrdersByStatus(symbol: string, status: string, limit: number): Promise<BackendSpotOrder[]> {
  const response = await request.instance.get<BackendSpotOrdersResponse>(backendApiUrl('/spot/orders'), {
    params: {
      pair_id: pairId(symbol),
      status,
      limit,
    },
  })
  return response.data.orders
}

function pairId(symbol: string): string {
  return symbol.replace('/', '-').replace('_', '-').toUpperCase()
}
