import { client, requestUrl } from './client'
import { asNumber, normalizeSymbol, splitSymbol } from '@/core/format'
import type { MarginProduct, WalletAccount } from '@/core/types'

export interface SpotOrderInput {
  symbol: string
  side: 'buy' | 'sell'
  type: 'limit' | 'market'
  price?: number
  quantity: number
}

export interface MarginOrderInput {
  productId: number
  side: 'long' | 'short'
  marginMode: 'isolated'
  leverage: number
  marginAmount: number
}

interface BackendMarginProduct {
  id: number
  symbol: string
  margin_asset_symbol?: string
  margin_mode?: string
  margin_modes?: string[] | string
  leverage_levels?: string[] | string
  max_leverage?: string | number
  min_margin?: string | number
}

interface BackendMarginTradingCapabilities {
  margin_modes?: string[] | string
  order_types?: string[] | string
}

export interface SpotOrder {
  id: string
  symbol: string
  side: 'buy' | 'sell'
  orderType: string
  price: number
  quantity: number
  filledQuantity: number
  status: string
  createdAt?: number
}

export interface MarginPosition {
  id: string
  productId: number
  pairId: number
  symbol: string
  direction: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  marginAmount: number
  notionalAmount: number
  leverage: number
  entryPrice: number
  realizedPnl: number
  interestAmount: number
  status: string
}

export interface MarginWallets {
  wallets: WalletAccount[]
  positions: MarginPosition[]
}

export async function placeSpotOrder(input: SpotOrderInput): Promise<void> {
  const payload: Record<string, string> = {
    pair_id: normalizeSymbol(input.symbol).replace(/(USDT|USDC|BTC|ETH|USD)$/, '-$1'),
    side: input.side,
    order_type: input.type,
    quantity: String(input.quantity),
    idempotency_key: createIdempotencyKey('mobile-spot'),
  }
  if (input.type === 'limit') {
    payload.price = String(input.price || 0)
  } else {
    payload.reference_price = String(input.price || 0)
  }
  await client.post(requestUrl('/spot/orders'), payload)
}

export async function fetchSpotOrders(symbol?: string, status?: string, limit = 30): Promise<SpotOrder[]> {
  const pair = symbol ? splitSymbol(symbol) : undefined
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/spot/orders'), {
    params: {
      pair_id: pair ? `${pair.base}-${pair.quote}` : undefined,
      status,
      limit,
    },
  })
  return (response.data.orders || []).map((order) => ({
    id: String(order.id),
    symbol: String(order.pair_id || order.symbol || ''),
    side: String(order.side || 'buy').toLowerCase() === 'sell' ? 'sell' : 'buy',
    orderType: String(order.order_type || 'limit'),
    price: asNumber(order.price ?? order.average_price),
    quantity: asNumber(order.quantity),
    filledQuantity: asNumber(order.filled_quantity),
    status: String(order.status || 'pending'),
    createdAt: normalizeTimestamp(order.created_at),
  }))
}

export async function cancelSpotOrder(orderId: string): Promise<void> {
  await client.delete(requestUrl(`/spot/orders/${encodeURIComponent(orderId)}`))
}

export async function fetchOpenSpotOrders(limit = 30): Promise<SpotOrder[]> {
  const pages = await Promise.all(['pending', 'open', 'partially_filled'].map((status) => fetchSpotOrders(undefined, status, limit)))
  return uniqueSpotOrders(pages.flat())
}

export async function fetchSpotOrderHistory(limit = 30): Promise<SpotOrder[]> {
  const pages = await Promise.all(['filled', 'cancelled', 'rejected'].map((status) => fetchSpotOrders(undefined, status, limit)))
  return uniqueSpotOrders(pages.flat())
}

export async function cancelAllSpotOrders(orderIds: string[]): Promise<void> {
  // 后端暂未提供现货批量撤单端点，移动端按当前委托逐笔撤销。
  const results = await Promise.allSettled(orderIds.map((orderId) => cancelSpotOrder(orderId)))
  const rejected = results.find((result): result is PromiseRejectedResult => result.status === 'rejected')
  if (rejected) throw rejected.reason
}

export async function fetchMarginProducts(): Promise<MarginProduct[]> {
  const response = await client.get<{ products?: BackendMarginProduct[]; capabilities?: BackendMarginTradingCapabilities }>(requestUrl('/margin/products'))
  return (response.data.products || []).map((product) => {
    const pair = splitSymbol(product.symbol)
    const modes = resolveMarginModes(response.data.capabilities?.margin_modes, product.margin_modes, product.margin_mode)
    const levels = parseLeverage(product.leverage_levels, product.max_leverage)
    return {
      id: product.id,
      symbol: `${pair.base}/${pair.quote}`,
      marginAssetSymbol: (product.margin_asset_symbol || pair.quote).toUpperCase(),
      marginMode: modes[0] || 'isolated',
      marginModes: modes,
      leverageLevels: levels,
      maxLeverage: asNumber(product.max_leverage, levels.at(-1) || 1),
      minMargin: asNumber(product.min_margin),
    }
  })
}

export async function placeMarginOrder(input: MarginOrderInput): Promise<void> {
  await client.post(requestUrl('/margin/positions'), {
    product_id: input.productId,
    direction: input.side,
    order_type: 'market',
    margin_mode: input.marginMode,
    margin_amount: String(input.marginAmount),
    leverage: String(input.leverage),
    idempotency_key: createIdempotencyKey('mobile-margin'),
  })
}

export async function fetchMarginPositions(status?: string, limit = 30): Promise<MarginPosition[]> {
  const response = await client.get<{ positions?: Array<Record<string, unknown>> }>(requestUrl('/margin/positions'), {
    params: { status, limit },
  })
  return (response.data.positions || []).map(mapMarginPosition)
}

export async function fetchMarginWallets(): Promise<MarginWallets> {
  const response = await client.get<{ wallets?: Array<Record<string, unknown>>; positions?: Array<Record<string, unknown>> }>(requestUrl('/margin/wallets'))
  return {
    wallets: (response.data.wallets || []).map((wallet) => ({
      assetId: asNumber(wallet.asset_id),
      symbol: String(wallet.asset_symbol || '').toUpperCase(),
      available: asNumber(wallet.available),
      frozen: asNumber(wallet.frozen),
      locked: asNumber(wallet.locked),
    })),
    positions: (response.data.positions || []).map(mapMarginPosition),
  }
}

export async function closeMarginPosition(positionId: string): Promise<void> {
  await client.post(requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/close`), {})
}

export async function cancelMarginPosition(positionId: string): Promise<void> {
  await client.post(requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/cancel`), {})
}

export async function closeAllMarginPositions(productId?: number): Promise<void> {
  await client.post(requestUrl('/margin/positions/close-all'), { product_id: productId || undefined })
}

export async function cancelAllMarginPositions(productId?: number): Promise<void> {
  await client.post(requestUrl('/margin/positions/cancel-all'), { product_id: productId || undefined })
}

export async function updateMarginLeverage(productId: number, leverage: number): Promise<void> {
  await client.patch(requestUrl(`/margin/settings/${productId}/leverage`), { leverage: String(leverage) })
}

export async function updateMarginMode(productId: number, mode: 'isolated'): Promise<void> {
  await client.patch(requestUrl(`/margin/settings/${productId}/mode`), { margin_mode: mode })
}

function parseModes(value: BackendMarginProduct['margin_modes'], fallback?: string): Array<'cross' | 'isolated'> {
  const values = Array.isArray(value) ? value : typeof value === 'string' ? value.split(',') : [fallback || 'cross']
  const normalized = values
    .map((item) => item.trim().toLowerCase())
    .filter((item): item is 'cross' | 'isolated' => item === 'cross' || item === 'isolated')
  return normalized.length ? [...new Set(normalized)] : ['cross']
}

function resolveMarginModes(
  capabilityModes: BackendMarginTradingCapabilities['margin_modes'],
  productModes: BackendMarginProduct['margin_modes'],
  fallback?: string,
): Array<'cross' | 'isolated'> {
  const configured = parseModes(productModes, fallback)
  if (!capabilityModes) return configured
  const supported = parseModes(capabilityModes, 'isolated')
  const usable = configured.filter((mode) => supported.includes(mode))
  return usable.length ? usable : supported
}

function parseLeverage(value: BackendMarginProduct['leverage_levels'], maxLeverage?: string | number): number[] {
  const values = Array.isArray(value) ? value : typeof value === 'string' ? value.split(',') : [maxLeverage || 1]
  return [...new Set(values.map((item) => asNumber(String(item).replace(/x$/i, ''))).filter((item) => item > 0))].sort((a, b) => a - b)
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}

function normalizeTimestamp(value: unknown): number | undefined {
  const timestamp = asNumber(value)
  if (!timestamp) return undefined
  return timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function mapMarginPosition(position: Record<string, unknown>): MarginPosition {
  return {
    id: String(position.id),
    productId: asNumber(position.product_id),
    pairId: asNumber(position.pair_id),
    symbol: String(position.symbol || position.pair_symbol || position.pair_id || ''),
    direction: String(position.direction || '').toLowerCase() === 'short' ? 'short' : 'long',
    marginMode: String(position.margin_mode || 'isolated').toLowerCase() === 'cross' ? 'cross' : 'isolated',
    marginAmount: asNumber(position.margin_amount),
    notionalAmount: asNumber(position.notional_amount),
    leverage: asNumber(position.leverage, 1),
    entryPrice: asNumber(position.entry_price),
    realizedPnl: asNumber(position.realized_pnl),
    interestAmount: asNumber(position.interest_amount),
    status: String(position.status || 'open'),
  }
}

function uniqueSpotOrders(orders: SpotOrder[]): SpotOrder[] {
  return [...new Map(orders.map((order) => [order.id, order])).values()]
    .sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0))
}
