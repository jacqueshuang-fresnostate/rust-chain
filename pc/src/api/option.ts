import request from './request'
import {
  backendApiUrl,
  mapSecondsOrdersToPcOrders,
  mapSecondsProductsToPcCycles,
  type BackendSecondsOrdersResponse,
  type BackendSecondsProduct,
  type BackendSecondsProductsResponse,
} from './backendAdapters'

export async function fetchOptionCoin(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  return { data: mapSecondsProductsToPcCycles(response.data) }
}

export async function submitOptionOrder(
  symbol: string,
  direction: 'BUY' | 'SELL',
  amount: number,
  periodSeconds: number,
  _currentPrice: number = 0,
): Promise<{ data: any }> {
  const product = await resolveSecondsProduct(symbol, periodSeconds)
  const response = await request.instance.post(backendApiUrl('/seconds-contracts/orders'), {
    product_id: product.id,
    direction: direction === 'SELL' ? 'down' : 'up',
    stake_amount: String(amount),
    idempotency_key: createOptionIdempotencyKey(),
  })

  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export function checkOrderSettlement(_currentPrice: number): [] {
  return []
}

export async function fetchOptionOrders(symbol: string, status: 'OPEN' | 'HISTORY', pageNo: number = 1, pageSize: number = 10): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsOrdersResponse>(backendApiUrl('/seconds-contracts/orders'))
  const filtered = response.data.orders.filter((order) => {
    const open = isOpenStatus(order.status)
    return symbolMatches(order.symbol, symbol) && (status === 'OPEN' ? open : !open)
  })
  const mapped = mapSecondsOrdersToPcOrders({ orders: filtered }).data

  return {
    data: {
      code: 0,
      message: 'success',
      data: {
        content: mapped.slice((pageNo - 1) * pageSize, pageNo * pageSize),
        totalElements: mapped.length,
      },
    },
  }
}

async function resolveSecondsProduct(symbol: string, periodSeconds: number): Promise<BackendSecondsProduct> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  const product = response.data.products.find((item) => {
    return item.status === 'active' && normalizeSymbol(item.symbol) === normalizeSymbol(symbol) && item.duration_seconds === periodSeconds
  })
  if (!product) throw new Error(`Seconds contract product unavailable: ${symbol} ${periodSeconds}s`)
  return product
}

function isOpenStatus(status: string): boolean {
  const normalized = status.toLowerCase()
  return normalized === 'open' || normalized === 'opened' || normalized === 'pending'
}

function symbolMatches(source?: string, expected?: string): boolean {
  return !expected || normalizeSymbol(source || '') === normalizeSymbol(expected)
}

function normalizeSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function createOptionIdempotencyKey(): string {
  return `pc-option-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
