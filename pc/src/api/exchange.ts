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
  const response = await request.instance.post<BackendSpotOrder>(
    backendApiUrl('/spot/orders'),
    mapPcSpotOrderRequest(params, createSpotIdempotencyKey()),
  )
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

function createSpotIdempotencyKey(): string {
  return `pc-spot-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
