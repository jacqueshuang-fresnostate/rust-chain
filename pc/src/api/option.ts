import request from './request'
import {
  backendApiUrl,
  mapSecondsOrdersToPcOrders,
  mapSecondsProductsToPcCycles,
  type BackendSecondsOrdersResponse,
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
    product_id: product.productId,
    duration_seconds: product.durationSeconds,
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

async function resolveSecondsProduct(symbol: string, periodSeconds: number): Promise<{ productId: number; durationSeconds: number }> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  for (const product of response.data.products) {
    if (product.status !== 'active' || normalizeSymbol(product.symbol) !== normalizeSymbol(symbol)) continue
    const cycles = product.cycles?.length
      ? product.cycles
      : [
          {
            id: product.id,
            product_id: product.id,
            duration_seconds: product.duration_seconds,
            payout_rate: product.payout_rate,
            min_stake: product.min_stake,
            max_stake: product.max_stake,
          },
        ]
    const cycle = cycles.find((item) => item.duration_seconds === periodSeconds)
    if (cycle) {
      return { productId: product.id, durationSeconds: cycle.duration_seconds }
    }
  }
  throw new Error(`Seconds contract product unavailable: ${symbol} ${periodSeconds}s`)
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
