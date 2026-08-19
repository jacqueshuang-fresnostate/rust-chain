import axios from 'axios'
import { client, requestUrl } from './client'
import { asNumber, normalizeSymbol, splitSymbol } from '@/core/format'
import { mapMarginProductMarginLimits } from '@/core/tradeForm'
import { parseMarginOrderTypes } from '@/core/marginOrder'
import type { MarginOrderType, MarginProduct, WalletAccount } from '@/core/types'

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
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: number
  orderType: MarginOrderType
  price?: string
  idempotencyKey?: string
}

export interface MarginUserSetting {
  leverage: number | null
  marginMode: 'cross' | 'isolated' | null
}

interface BackendMarginProduct {
  id: number
  pair_id?: number
  symbol: string
  margin_asset?: string | number
  margin_asset_symbol?: string
  logo_url?: string | null
  margin_mode?: string
  margin_modes?: string[] | string
  leverage_levels?: string[] | string
  max_leverage?: string | number
  min_margin?: string | number
  max_margin?: string | number | null
  price_precision?: string | number
  maintenance_margin_rate?: string | number
  hourly_interest_rate?: string | number
}

interface BackendMarginTradingCapabilities {
  margin_modes?: string[] | string
  order_types?: string[] | string
  take_profit_stop_loss?: boolean
  strategy_orders?: boolean
  bulk_close?: boolean
  position_risk?: boolean
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
  marginAssetId: number
  direction: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  marginAmount: number
  notionalAmount: number
  borrowedAmount: number
  leverage: number
  orderType: MarginOrderType
  entryPrice: number | null
  /** Preserve the backend DECIMAL text so a pending order's exact trigger intent is not rounded by JS. */
  limitPrice: string | null
  realizedPnl: number
  interestAmount: number
  status: string
}

export interface MarginWallets {
  wallets: WalletAccount[]
  positions: MarginPosition[]
  crossAccounts: MarginCrossAccount[]
}

export interface MarginCrossAccount {
  marginAssetId: number
  status: string
  equity: number
  unrealizedPnl: number
  interestAmount: number
  maintenanceMargin: number
  marginRatio: number | null
}

export interface MarginPositionRisk {
  positionId: string
  pairId: number
  symbol: string
  marginAssetId: number
  direction: 'long' | 'short'
  marginAmount: number
  notionalAmount: number
  interestAmount: number
  entryPrice: number
  markPrice: number
  maintenanceMarginRate: number
  unrealizedPnl: number
  equity: number
  maintenanceMargin: number
  positionQuantity: number
  returnRate: number | null
  marginRatio: number | null
  estimatedLiquidationPrice: number | null
  liquidationDistanceRate: number | null
  shouldLiquidate: boolean
  observedAt?: number
}

export interface MarginBatchActionFailure {
  id: string
  code: string
  message: string
}

export interface MarginBatchActionResult {
  positions: MarginPosition[]
  failures: MarginBatchActionFailure[]
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
  const orderTypes = parseMarginOrderTypes(response.data.capabilities?.order_types)
  return (response.data.products || []).map((product) => {
    const pair = splitSymbol(product.symbol)
    const modes = resolveMarginModes(response.data.capabilities?.margin_modes, product.margin_modes, product.margin_mode)
    const levels = parseLeverage(product.leverage_levels, product.max_leverage)
    const marginLimits = mapMarginProductMarginLimits(product)
    return {
      id: product.id,
      pairId: asNumber(product.pair_id),
      symbol: `${pair.base}/${pair.quote}`,
      marginAssetId: asNumber(product.margin_asset),
      marginAssetSymbol: (product.margin_asset_symbol || pair.quote).toUpperCase(),
      logoUrl: String(product.logo_url || '').trim() || undefined,
      marginMode: modes[0] || 'isolated',
      marginModes: modes,
      orderTypes: [...orderTypes],
      pricePrecision: nonNegativeInteger(product.price_precision),
      leverageLevels: levels,
      maxLeverage: asNumber(product.max_leverage, levels.at(-1) || 1),
      minMargin: marginLimits.minMargin,
      maxMargin: marginLimits.maxMargin,
      maintenanceMarginRate: asNumber(product.maintenance_margin_rate),
      hourlyInterestRate: asNumber(product.hourly_interest_rate),
      takeProfitStopLossSupported: response.data.capabilities?.take_profit_stop_loss === true,
      strategyOrdersSupported: response.data.capabilities?.strategy_orders === true,
      bulkCloseSupported: response.data.capabilities?.bulk_close === true,
      positionRiskSupported: response.data.capabilities?.position_risk === true,
    }
  })
}

export async function placeMarginOrder(input: MarginOrderInput): Promise<void> {
  const payload: Record<string, number | string> = {
    product_id: input.productId,
    direction: input.side,
    order_type: input.orderType,
    margin_mode: input.marginMode,
    margin_amount: String(input.marginAmount),
    leverage: String(input.leverage),
    idempotency_key: input.idempotencyKey || createMarginOrderIdempotencyKey(),
  }
  if (input.orderType === 'limit') {
    const price = input.price?.trim()
    if (!price) throw new TypeError('margin limit order requires a frozen price')
    payload.price = price
  }
  await client.post(requestUrl('/margin/positions'), payload)
}

export function createMarginOrderIdempotencyKey(): string {
  return createIdempotencyKey('mobile-margin')
}

export async function fetchMarginPositions(status?: string, limit = 30): Promise<MarginPosition[]> {
  const response = await client.get<{ positions?: Array<Record<string, unknown>> }>(requestUrl('/margin/positions'), {
    params: { status, limit },
  })
  return (response.data.positions || []).map(mapMarginPosition)
}

export async function fetchMarginWallets(): Promise<MarginWallets> {
  const response = await client.get<{
    wallets?: Array<Record<string, unknown>>
    positions?: Array<Record<string, unknown>>
    cross_accounts?: Array<Record<string, unknown>>
  }>(requestUrl('/margin/wallets'))
  return {
    wallets: (response.data.wallets || []).map((wallet) => ({
      assetId: asNumber(wallet.asset_id),
      symbol: String(wallet.asset_symbol || '').toUpperCase(),
      logoUrl: String(wallet.logo_url || '').trim() || undefined,
      marginTransferEnabled: wallet.margin_transfer_enabled !== false,
      available: asNumber(wallet.available),
      frozen: asNumber(wallet.frozen),
      locked: asNumber(wallet.locked),
    })),
    positions: (response.data.positions || []).map(mapMarginPosition),
    crossAccounts: (response.data.cross_accounts || []).map((account) => ({
      marginAssetId: asNumber(account.margin_asset),
      status: String(account.status || ''),
      equity: asNumber(account.equity),
      unrealizedPnl: asNumber(account.unrealized_pnl),
      interestAmount: asNumber(account.interest_amount),
      maintenanceMargin: asNumber(account.maintenance_margin),
      marginRatio: optionalNumber(account.margin_ratio),
    })),
  }
}

/**
 * 读取单个已成交仓位的服务端即时风险快照。
 *
 * 页面不得用该接口替代钱包/仓位列表，也不得把请求失败解释为零风险；失败时保留仓位并显示
 * 占位符。所有后端 DECIMAL 仅在这一展示适配层转换为 number，不再参与提交请求。
 */
export async function fetchMarginPositionRisk(positionId: string): Promise<MarginPositionRisk> {
  const response = await client.get<{ risk?: Record<string, unknown> }>(
    requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/risk`),
  )
  const risk = response.data.risk || {}
  return {
    positionId: String(risk.position_id ?? positionId),
    pairId: asNumber(risk.pair_id),
    symbol: String(risk.symbol || ''),
    marginAssetId: asNumber(risk.margin_asset),
    direction: String(risk.direction || '').toLowerCase() === 'short' ? 'short' : 'long',
    marginAmount: asNumber(risk.margin_amount),
    notionalAmount: asNumber(risk.notional_amount),
    interestAmount: asNumber(risk.interest_amount),
    entryPrice: asNumber(risk.entry_price),
    markPrice: asNumber(risk.mark_price),
    maintenanceMarginRate: asNumber(risk.maintenance_margin_rate),
    unrealizedPnl: asNumber(risk.unrealized_pnl ?? risk.realized_pnl),
    equity: asNumber(risk.equity),
    maintenanceMargin: asNumber(risk.maintenance_margin),
    positionQuantity: asNumber(risk.position_quantity),
    returnRate: optionalNumber(risk.return_rate),
    marginRatio: optionalNumber(risk.margin_ratio),
    estimatedLiquidationPrice: optionalNumber(risk.estimated_liquidation_price),
    liquidationDistanceRate: optionalNumber(risk.liquidation_distance_rate),
    shouldLiquidate: risk.should_liquidate === true,
    observedAt: normalizeTimestamp(risk.observed_at),
  }
}

export async function closeMarginPosition(positionId: string): Promise<void> {
  await client.post(requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/close`), {})
}

export async function cancelMarginPosition(positionId: string): Promise<void> {
  await client.post(requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/cancel`), {})
}

export async function closeAllMarginPositions(productId?: number): Promise<MarginBatchActionResult> {
  const response = await client.post<{
    positions?: Array<Record<string, unknown>>
    failures?: Array<Record<string, unknown>>
  }>(requestUrl('/margin/positions/close-all'), { product_id: productId || undefined })
  return mapMarginBatchAction(response.data)
}

export async function cancelAllMarginPositions(productId?: number): Promise<MarginBatchActionResult> {
  const response = await client.post<{
    positions?: Array<Record<string, unknown>>
    failures?: Array<Record<string, unknown>>
  }>(requestUrl('/margin/positions/cancel-all'), { product_id: productId || undefined })
  return mapMarginBatchAction(response.data)
}

export async function updateMarginLeverage(productId: number, leverage: number): Promise<void> {
  await client.patch(requestUrl(`/margin/settings/${productId}/leverage`), { leverage: String(leverage) })
}

/**
 * 读取当前用户针对单个杠杆产品保存的模式与倍数。
 *
 * 用户从未修改过该产品时后端返回 404，这不是页面错误：调用方应继续使用产品配置中的
 * 默认模式和可选倍数。其他网络或服务端错误继续抛出，避免把真实故障误判成“未设置”。
 */
export async function fetchMarginSetting(productId: number): Promise<MarginUserSetting> {
  try {
    const response = await client.get<{ leverage?: string | number | null; margin_mode?: string | null }>(
      requestUrl(`/margin/settings/${productId}`),
    )
    const leverage = asNumber(response.data.leverage)
    const rawMode = response.data.margin_mode?.trim().toLowerCase()
    return {
      leverage: leverage > 0 ? leverage : null,
      marginMode: rawMode === 'cross' || rawMode === 'isolated' ? rawMode : null,
    }
  } catch (error) {
    if (axios.isAxiosError(error) && error.response?.status === 404) {
      return { leverage: null, marginMode: null }
    }
    throw error
  }
}

export async function updateMarginMode(productId: number, mode: 'cross' | 'isolated'): Promise<void> {
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
    marginAssetId: asNumber(position.margin_asset),
    direction: String(position.direction || '').toLowerCase() === 'short' ? 'short' : 'long',
    marginMode: String(position.margin_mode || 'isolated').toLowerCase() === 'cross' ? 'cross' : 'isolated',
    marginAmount: asNumber(position.margin_amount),
    notionalAmount: asNumber(position.notional_amount),
    borrowedAmount: asNumber(position.borrowed_amount),
    leverage: asNumber(position.leverage, 1),
    orderType: String(position.order_type || '').trim().toLowerCase() === 'limit' ? 'limit' : 'market',
    entryPrice: optionalNumber(position.entry_price),
    limitPrice: optionalDecimalString(position.limit_price),
    realizedPnl: asNumber(position.realized_pnl),
    interestAmount: asNumber(position.interest_amount),
    status: String(position.status || 'open'),
  }
}

function mapMarginBatchAction(payload: {
  positions?: Array<Record<string, unknown>>
  failures?: Array<Record<string, unknown>>
}): MarginBatchActionResult {
  return {
    positions: (payload.positions || []).map(mapMarginPosition),
    failures: (payload.failures || []).map((failure) => ({
      id: String(failure.id || ''),
      code: String(failure.code || ''),
      message: String(failure.message || ''),
    })),
  }
}

function optionalNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === '') return null
  const parsed = asNumber(value, Number.NaN)
  return Number.isFinite(parsed) ? parsed : null
}

function optionalDecimalString(value: unknown): string | null {
  if (value === null || value === undefined || value === '') return null
  const normalized = String(value).trim()
  if (!normalized || !Number.isFinite(Number(normalized))) return null
  return normalized
}

function nonNegativeInteger(value: unknown): number | null {
  const parsed = typeof value === 'string' && value.trim() ? Number(value) : value
  return typeof parsed === 'number' && Number.isInteger(parsed) && parsed >= 0 ? parsed : null
}

function uniqueSpotOrders(orders: SpotOrder[]): SpotOrder[] {
  return [...new Map(orders.map((order) => [order.id, order])).values()]
    .sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0))
}
