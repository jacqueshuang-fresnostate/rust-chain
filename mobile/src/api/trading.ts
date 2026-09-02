import axios from 'axios'
import { client, requestUrl } from './client'
import { asNumber, normalizeSymbol, splitSymbol } from '@/core/format'
import { parseMarginOrderTypes } from '@/core/marginOrder'
import {
  mapMarginCrossAccountRisk,
  type MarginCrossAccountRisk,
} from '@/core/marginRiskMetrics'
import { mapMarginUserLeverageSetting } from '@/core/marginLeverage'
import type { MarginOrderType, MarginProduct, WalletAccount } from '@/core/types'
import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency'
import {
  normalizeDecimalText,
  requiredDecimalText,
  type DecimalText,
} from '@/core/decimal'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'

const spotOrderIdempotencyKeys = new RetryStableIdempotencyKeys('mobile-spot')

const FINANCIAL_DECIMAL_CONSTRAINTS = {
  maxIntegerDigits: 20,
  maxScale: 18,
} as const

export class TradingFinancialContractError extends TypeError {
  constructor(field: string) {
    super(`invalid trading financial ${field}`)
    this.name = 'TradingFinancialContractError'
  }
}

export type {
  MarginCrossAccountPriceAssumption,
  MarginCrossAccountRisk,
} from '@/core/marginRiskMetrics'

export interface SpotOrderInput {
  symbol: string
  side: 'buy' | 'sell'
  type: 'limit' | 'market'
  price?: DecimalText
  quantity: DecimalText
}

export interface MarginOrderInput {
  productId: number
  side: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: DecimalText
  orderType: MarginOrderType
  price?: DecimalText
  idempotencyKey?: string
}

export interface MarginUserSetting {
  leverage: number | null
  longLeverage: number | null
  shortLeverage: number | null
  marginMode: 'cross' | 'isolated' | null
}

export interface MarginDirectionalLeverageInput {
  longLeverage: number
  shortLeverage: number
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
  priceText: DecimalText | null
  averagePriceText: DecimalText | null
  quantityText: DecimalText
  filledQuantityText: DecimalText
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
  marginAmountText: DecimalText
  notionalAmountText: DecimalText
  borrowedAmountText: DecimalText
  leverage: number
  orderType: MarginOrderType
  entryPrice: number | null
  entryPriceText: DecimalText | null
  exitPrice: number | null
  exitPriceText: DecimalText | null
  /** Preserve the backend DECIMAL text so a pending order's exact trigger intent is not rounded by JS. */
  limitPrice: DecimalText | null
  limitPriceText: DecimalText | null
  realizedPnl: number
  interestAmount: number
  realizedPnlText: DecimalText | null
  interestAmountText: DecimalText
  status: string
  openedAt?: number
  createdAt?: number
  closedAt?: number
}

export interface MarginPositionExecution {
  id: string
  positionId: string
  idempotencyKey: string
  closePercentage: number
  closeMarginAmountText: DecimalText
  closeNotionalAmountText: DecimalText
  closeBorrowedAmountText: DecimalText
  closeInterestAmountText: DecimalText
  exitPriceText: DecimalText
  realizedPnlText: DecimalText
  settlementAmountText: DecimalText
  fullyClosed: boolean
  createdAt: number
}

export interface MarginWallets {
  wallets: MarginWalletAccount[]
  positions: MarginPosition[]
  crossAccounts: MarginCrossAccount[]
}

export interface MarginWalletAccount extends WalletAccount {
  availableText: DecimalText
  frozenText: DecimalText
  lockedText: DecimalText
  maxTransferableToSpot: number
  maxTransferableToSpotText: DecimalText
  transferRiskEquity: number | null
  transferRiskEquityText: DecimalText | null
  transferRiskMaintenanceMargin: number | null
  transferRiskMaintenanceMarginText: DecimalText | null
}

export interface MarginCrossAccount {
  marginAssetId: number
  status: string
  equity: number
  unrealizedPnl: number
  interestAmount: number
  maintenanceMargin: number
  marginRatio: number | null
  equityText: DecimalText
  unrealizedPnlText: DecimalText
  interestAmountText: DecimalText
  maintenanceMarginText: DecimalText
  marginRatioText: DecimalText | null
}

export interface MarginProductFinancialAuthority extends MarginProduct {
  minMarginText: DecimalText
  maxMarginText: DecimalText | null
  maintenanceMarginRateText: DecimalText
  hourlyInterestRateText: DecimalText
}

export interface MarginCrossAccountRiskFinancialAuthority extends MarginCrossAccountRisk {
  equityText: DecimalText
  maintenanceMarginText: DecimalText
  liquidationBufferText: DecimalText
  marginRatioText: DecimalText | null
  unrealizedPnlText: DecimalText
  interestAmountText: DecimalText
  netQuantityText: DecimalText
  grossQuantityText: DecimalText
  conditionalLiquidationPriceText: DecimalText | null
  conditionalLiquidationDistanceRateText: DecimalText | null
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
  maintenanceMarginRate: number | null
  unrealizedPnl: number
  equity: number
  maintenanceMargin: number
  positionQuantity: number
  returnRate: number | null
  marginRatio: number | null
  estimatedLiquidationPrice: number | null
  liquidationDistanceRate: number | null
  marginAmountText: DecimalText
  notionalAmountText: DecimalText
  interestAmountText: DecimalText
  entryPriceText: DecimalText
  markPriceText: DecimalText
  maintenanceMarginRateText: DecimalText
  unrealizedPnlText: DecimalText
  equityText: DecimalText
  maintenanceMarginText: DecimalText
  positionQuantityText: DecimalText
  returnRateText: DecimalText | null
  marginRatioText: DecimalText | null
  estimatedLiquidationPriceText: DecimalText | null
  liquidationDistanceRateText: DecimalText | null
  shouldLiquidate: boolean
  observedAt?: number
  crossAccountRisk?: MarginCrossAccountRiskFinancialAuthority
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

export interface SpotCancelAllOptions {
  pairId?: string
  signal?: AbortSignal
}

export interface SpotCancelAllFailure {
  id: string
  code: string
  message: string
}

export interface SpotCancelAllResult {
  orders: SpotOrder[]
  failures: SpotCancelAllFailure[]
}

export interface MarginCloseInput {
  percentage: number
  idempotencyKey: string
}

export async function placeSpotOrder(input: SpotOrderInput): Promise<void> {
  const businessIntent: Record<string, string> = {
    pair_id: normalizeSymbol(input.symbol).replace(/(USDT|USDC|BTC|ETH|USD)$/, '-$1'),
    side: input.side,
    order_type: input.type,
    quantity: normalizeDecimalText(input.quantity),
  }
  if (!input.price) {
    throw new TradingFinancialContractError(
      input.type === 'limit' ? 'spot order price' : 'spot order reference_price',
    )
  }
  const requestPrice = normalizeDecimalText(input.price)
  if (input.type === 'limit') {
    businessIntent.price = requestPrice
  } else {
    businessIntent.reference_price = requestPrice
  }
  const intent = canonicalRequestIntent(businessIntent)
  const idempotencyKey = spotOrderIdempotencyKeys.acquire(intent)
  await client.post(requestUrl('/spot/orders'), {
    ...businessIntent,
    idempotency_key: idempotencyKey,
  })
  spotOrderIdempotencyKeys.complete(intent, idempotencyKey)
}

export async function fetchSpotOrders(
  symbol?: string,
  status?: string,
  limit = 30,
  signal?: AbortSignal,
): Promise<SpotOrder[]> {
  const pair = symbol ? splitSymbol(symbol) : undefined
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/spot/orders'), {
    params: {
      pair_id: pair ? `${pair.base}-${pair.quote}` : undefined,
      status,
      limit,
    },
    signal,
  })
  return (response.data.orders || []).map(mapSpotOrder)
}

function mapSpotOrder(order: Record<string, unknown>): SpotOrder {
  const priceText = nullableTradingDecimal(order.price, 'spot order price', {
    allowNegative: false,
  })
  const averagePriceText = nullableTradingDecimal(order.average_price, 'spot order average_price', {
    allowNegative: false,
  })
  const quantityText = tradingDecimal(order.quantity, 'spot order quantity', {
    allowNegative: false,
  })
  const filledQuantityText = tradingDecimal(order.filled_quantity, 'spot order filled_quantity', {
    allowNegative: false,
  })
  return {
    id: String(order.id),
    symbol: String(order.pair_id || order.symbol || ''),
    side: String(order.side || 'buy').toLowerCase() === 'sell' ? 'sell' : 'buy',
    orderType: String(order.order_type || 'limit'),
    price: priceText ? decimalDisplayNumber(priceText, 'spot order price') : Number.NaN,
    quantity: decimalDisplayNumber(quantityText, 'spot order quantity'),
    filledQuantity: decimalDisplayNumber(filledQuantityText, 'spot order filled_quantity'),
    priceText,
    averagePriceText,
    quantityText,
    filledQuantityText,
    status: String(order.status || 'pending'),
    createdAt: normalizeTimestamp(order.created_at),
  }
}

export async function cancelSpotOrder(orderId: string): Promise<void> {
  await client.delete(requestUrl(`/spot/orders/${encodeURIComponent(orderId)}`))
}

export async function fetchOpenSpotOrders(limit = 30, signal?: AbortSignal): Promise<SpotOrder[]> {
  const pages = await Promise.all(['pending', 'open', 'partially_filled'].map(
    (status) => fetchSpotOrders(undefined, status, limit, signal),
  ))
  return uniqueSpotOrders(pages.flat())
}

export async function fetchSpotOrderHistory(limit = 30, signal?: AbortSignal): Promise<SpotOrder[]> {
  const pages = await Promise.all(['filled', 'cancelled', 'rejected'].map(
    (status) => fetchSpotOrders(undefined, status, limit, signal),
  ))
  return uniqueSpotOrders(pages.flat())
}

export async function cancelAllSpotOrders(
  options: SpotCancelAllOptions = {},
): Promise<SpotCancelAllResult> {
  const response = await client.delete<{
    orders?: unknown
    failures?: unknown
  }>(requestUrl('/spot/orders'), {
    params: { pair_id: options.pairId?.trim() || undefined },
    signal: options.signal,
  })
  if (!Array.isArray(response.data.orders) || !Array.isArray(response.data.failures)) {
    throw new Error('Spot batch-cancel response is malformed')
  }
  return {
    orders: response.data.orders.map((order) => {
      if (!order || typeof order !== 'object' || Array.isArray(order)) {
        throw new Error('Spot batch-cancel order is malformed')
      }
      return mapSpotOrder(order as Record<string, unknown>)
    }),
    failures: response.data.failures.map((failure) => {
      if (!failure || typeof failure !== 'object' || Array.isArray(failure)) {
        throw new Error('Spot batch-cancel failure is malformed')
      }
      const row = failure as Record<string, unknown>
      return {
        id: String(row.id || '').trim(),
        code: String(row.code || '').trim(),
        message: String(row.message || '').trim(),
      }
    }),
  }
}

export async function fetchMarginProducts(options: ReferenceRequestOptions = {}): Promise<MarginProductFinancialAuthority[]> {
  const url = requestUrl('/margin/products')
  // 产品能力目录不含用户仓位或钱包数据，因此公开缓存键不需要 token。
  return referenceRequestRegistry.request(createReferenceRequestKey(url), 60_000, async () => {
    const response = await client.get<{ products?: BackendMarginProduct[]; capabilities?: BackendMarginTradingCapabilities }>(url)
    const orderTypes = parseMarginOrderTypes(response.data.capabilities?.order_types)
    return (response.data.products || []).map((product) => {
      const pair = splitSymbol(product.symbol)
      const modes = resolveMarginModes(response.data.capabilities?.margin_modes, product.margin_modes, product.margin_mode)
      const levels = parseLeverage(product.leverage_levels, product.max_leverage)
      const minMarginText = tradingDecimal(product.min_margin, 'margin product min_margin', {
        allowNegative: false,
        allowZero: false,
      })
      const maxMarginText = nullableTradingDecimal(product.max_margin, 'margin product max_margin', {
        allowNegative: false,
        allowZero: false,
      })
      const maintenanceMarginRateText = tradingDecimal(
        product.maintenance_margin_rate,
        'margin product maintenance_margin_rate',
        { allowNegative: false },
      )
      const hourlyInterestRateText = tradingDecimal(
        product.hourly_interest_rate,
        'margin product hourly_interest_rate',
        { allowNegative: false },
      )
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
        minMargin: decimalDisplayNumber(minMarginText, 'margin product min_margin'),
        maxMargin: nullableDecimalDisplayNumber(maxMarginText),
        minMarginText,
        maxMarginText,
        maintenanceMarginRate: decimalDisplayNumber(maintenanceMarginRateText, 'margin product maintenance_margin_rate'),
        maintenanceMarginRateText,
        hourlyInterestRate: decimalDisplayNumber(hourlyInterestRateText, 'margin product hourly_interest_rate'),
        hourlyInterestRateText,
        takeProfitStopLossSupported: response.data.capabilities?.take_profit_stop_loss === true,
        strategyOrdersSupported: response.data.capabilities?.strategy_orders === true,
        bulkCloseSupported: response.data.capabilities?.bulk_close === true,
        positionRiskSupported: response.data.capabilities?.position_risk === true,
      }
    })
  }, options)
}

export async function placeMarginOrder(input: MarginOrderInput): Promise<void> {
  const payload: Record<string, number | string> = {
    product_id: input.productId,
    direction: input.side,
    order_type: input.orderType,
    margin_mode: input.marginMode,
    margin_amount: normalizeDecimalText(input.marginAmount),
    leverage: String(input.leverage),
    idempotency_key: input.idempotencyKey || createMarginOrderIdempotencyKey(),
  }
  if (input.orderType === 'limit') {
    const price = input.price ? normalizeDecimalText(input.price) : undefined
    if (!price) throw new TypeError('margin limit order requires a frozen price')
    payload.price = price
  }
  await client.post(requestUrl('/margin/positions'), payload)
}

export function createMarginOrderIdempotencyKey(): string {
  return createIdempotencyKey('mobile-margin')
}

export async function fetchMarginPositions(
  status?: string,
  limit = 30,
  signal?: AbortSignal,
): Promise<MarginPosition[]> {
  const response = await client.get<{ positions?: Array<Record<string, unknown>> }>(requestUrl('/margin/positions'), {
    params: { status, limit },
    signal,
  })
  return (response.data.positions || []).map(mapMarginPosition)
}

export async function fetchMarginPosition(
  positionId: string,
  signal?: AbortSignal,
): Promise<MarginPosition> {
  const response = await client.get<{ position?: Record<string, unknown> }>(
    requestUrl(`/margin/positions/${encodeURIComponent(positionId)}`),
    { signal },
  )
  if (!response.data.position) throw new TradingFinancialContractError('margin position')
  const position = mapMarginPosition(response.data.position)
  if (position.id !== positionId) {
    throw new TradingFinancialContractError('margin position identity')
  }
  return position
}

export async function fetchMarginPositionExecutions(
  positionId: string,
  signal?: AbortSignal,
): Promise<MarginPositionExecution[]> {
  const response = await client.get<{ executions?: unknown }>(
    requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/executions`),
    { signal },
  )
  if (!Array.isArray(response.data.executions)) {
    throw new TradingFinancialContractError('margin position executions envelope')
  }
  return response.data.executions.map((execution) => {
    if (!execution || typeof execution !== 'object' || Array.isArray(execution)) {
      throw new TradingFinancialContractError('margin position execution')
    }
    const mapped = mapMarginPositionExecution(execution as Record<string, unknown>)
    if (mapped.positionId !== positionId) {
      throw new TradingFinancialContractError('margin position execution scope')
    }
    return mapped
  })
}

export async function fetchMarginWallets(signal?: AbortSignal): Promise<MarginWallets> {
  const response = await client.get<{
    wallets?: Array<Record<string, unknown>>
    positions?: Array<Record<string, unknown>>
    cross_accounts?: Array<Record<string, unknown>>
  }>(requestUrl('/margin/wallets'), { signal })
  if (!Array.isArray(response.data.wallets)
    || !Array.isArray(response.data.positions)
    || !Array.isArray(response.data.cross_accounts)) {
    throw new TradingFinancialContractError('margin wallets envelope')
  }
  return {
    wallets: response.data.wallets.map(mapMarginWallet),
    positions: response.data.positions.map(mapMarginPosition),
    crossAccounts: response.data.cross_accounts.map(mapMarginCrossAccount),
  }
}

/**
 * 读取单个已成交仓位的服务端即时风险快照。
 *
 * 页面不得用该接口替代钱包/仓位列表，也不得把请求失败解释为零风险；失败时保留仓位并显示
 * 占位符。所有后端 DECIMAL 仅在这一展示适配层转换为 number，不再参与提交请求。
 */
export async function fetchMarginPositionRisk(
  positionId: string,
  signal?: AbortSignal,
): Promise<MarginPositionRisk> {
  const response = await client.get<{ risk?: Record<string, unknown> }>(
    requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/risk`),
    { signal },
  )
  if (!response.data.risk) throw new TradingFinancialContractError('position risk')
  const risk = response.data.risk
  const marginAmountText = nonNegativeTradingDecimal(risk.margin_amount, 'position risk margin_amount')
  const notionalAmountText = nonNegativeTradingDecimal(risk.notional_amount, 'position risk notional_amount')
  const interestAmountText = nonNegativeTradingDecimal(risk.interest_amount, 'position risk interest_amount')
  const entryPriceText = nonNegativeTradingDecimal(risk.entry_price, 'position risk entry_price')
  const markPriceText = nonNegativeTradingDecimal(risk.mark_price, 'position risk mark_price')
  const maintenanceMarginRateText = nonNegativeTradingDecimal(risk.maintenance_margin_rate, 'position risk maintenance_margin_rate')
  const unrealizedPnlText = tradingDecimal(risk.unrealized_pnl ?? risk.realized_pnl, 'position risk unrealized_pnl')
  const equityText = tradingDecimal(risk.equity, 'position risk equity')
  const maintenanceMarginText = nonNegativeTradingDecimal(risk.maintenance_margin, 'position risk maintenance_margin')
  const positionQuantityText = nonNegativeTradingDecimal(risk.position_quantity, 'position risk position_quantity')
  const returnRateText = nullableTradingDecimal(risk.return_rate, 'position risk return_rate')
  const marginRatioText = nullableTradingDecimal(risk.margin_ratio, 'position risk margin_ratio')
  const estimatedLiquidationPriceText = nullableTradingDecimal(
    risk.estimated_liquidation_price,
    'position risk estimated_liquidation_price',
    { allowNegative: false, allowZero: false },
  )
  const liquidationDistanceRateText = nullableTradingDecimal(
    risk.liquidation_distance_rate,
    'position risk liquidation_distance_rate',
    { allowNegative: false },
  )
  return {
    positionId: String(risk.position_id ?? positionId),
    pairId: asNumber(risk.pair_id),
    symbol: String(risk.symbol || ''),
    marginAssetId: asNumber(risk.margin_asset),
    direction: String(risk.direction || '').toLowerCase() === 'short' ? 'short' : 'long',
    marginAmount: decimalDisplayNumber(marginAmountText, 'position risk margin_amount'),
    notionalAmount: decimalDisplayNumber(notionalAmountText, 'position risk notional_amount'),
    interestAmount: decimalDisplayNumber(interestAmountText, 'position risk interest_amount'),
    entryPrice: decimalDisplayNumber(entryPriceText, 'position risk entry_price'),
    markPrice: decimalDisplayNumber(markPriceText, 'position risk mark_price'),
    maintenanceMarginRate: decimalDisplayNumber(maintenanceMarginRateText, 'position risk maintenance_margin_rate'),
    unrealizedPnl: decimalDisplayNumber(unrealizedPnlText, 'position risk unrealized_pnl'),
    equity: decimalDisplayNumber(equityText, 'position risk equity'),
    maintenanceMargin: decimalDisplayNumber(maintenanceMarginText, 'position risk maintenance_margin'),
    positionQuantity: decimalDisplayNumber(positionQuantityText, 'position risk position_quantity'),
    returnRate: nullableDecimalDisplayNumber(returnRateText),
    marginRatio: nullableDecimalDisplayNumber(marginRatioText),
    estimatedLiquidationPrice: nullableDecimalDisplayNumber(estimatedLiquidationPriceText),
    liquidationDistanceRate: nullableDecimalDisplayNumber(liquidationDistanceRateText),
    marginAmountText,
    notionalAmountText,
    interestAmountText,
    entryPriceText,
    markPriceText,
    maintenanceMarginRateText,
    unrealizedPnlText,
    equityText,
    maintenanceMarginText,
    positionQuantityText,
    returnRateText,
    marginRatioText,
    estimatedLiquidationPriceText,
    liquidationDistanceRateText,
    shouldLiquidate: risk.should_liquidate === true,
    observedAt: normalizeTimestamp(risk.observed_at),
    crossAccountRisk: mapStrictMarginCrossAccountRisk(risk.cross_account_risk),
  }
}

export function createMarginCloseIdempotencyKey(): string {
  return createIdempotencyKey('mobile-margin-close')
}

export async function closeMarginPosition(
  positionId: string,
  input?: MarginCloseInput,
): Promise<void> {
  await client.post(
    requestUrl(`/margin/positions/${encodeURIComponent(positionId)}/close`),
    input
      ? {
          percentage: input.percentage,
          idempotency_key: input.idempotencyKey,
        }
      : {},
  )
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

export async function updateMarginLeverage(
  productId: number,
  leverage: number | MarginDirectionalLeverageInput,
): Promise<void> {
  const payload = typeof leverage === 'number'
    ? { leverage: String(leverage) }
    : {
        long_leverage: String(leverage.longLeverage),
        short_leverage: String(leverage.shortLeverage),
      }
  await client.patch(requestUrl(`/margin/settings/${productId}/leverage`), payload)
}

/**
 * 读取当前用户针对单个杠杆产品保存的模式与倍数。
 *
 * 用户从未修改过该产品时后端返回 404，这不是页面错误：调用方应继续使用产品配置中的
 * 默认模式和可选倍数。其他网络或服务端错误继续抛出，避免把真实故障误判成“未设置”。
 */
export async function fetchMarginSetting(productId: number): Promise<MarginUserSetting> {
  try {
    const response = await client.get<{
      leverage?: string | number | null
      long_leverage?: string | number | null
      short_leverage?: string | number | null
      margin_mode?: string | null
    }>(
      requestUrl(`/margin/settings/${productId}`),
    )
    const leverageSetting = mapMarginUserLeverageSetting(response.data)
    const rawMode = response.data.margin_mode?.trim().toLowerCase()
    return {
      ...leverageSetting,
      marginMode: rawMode === 'cross' || rawMode === 'isolated' ? rawMode : null,
    }
  } catch (error) {
    if (axios.isAxiosError(error) && error.response?.status === 404) {
      return { leverage: null, longLeverage: null, shortLeverage: null, marginMode: null }
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

function normalizeTimestamp(value: unknown, field?: string): number | undefined {
  if (value === null || value === undefined || value === '') return undefined
  const timestamp = typeof value === 'number'
    ? value
    : typeof value === 'string' && value.trim() ? Number(value) : Number.NaN
  const normalized = timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
  if (!Number.isSafeInteger(normalized) || normalized <= 0) {
    if (field) throw new TradingFinancialContractError(field)
    return undefined
  }
  return normalized
}

export function mapMarginPosition(position: Record<string, unknown>): MarginPosition {
  const marginAmountText = nonNegativeTradingDecimal(position.margin_amount, 'margin position margin_amount')
  const notionalAmountText = nonNegativeTradingDecimal(position.notional_amount, 'margin position notional_amount')
  const borrowedAmountText = nonNegativeTradingDecimal(position.borrowed_amount, 'margin position borrowed_amount')
  const entryPriceText = nullableTradingDecimal(position.entry_price, 'margin position entry_price', {
    allowNegative: false,
    allowZero: false,
  })
  const exitPriceText = nullableTradingDecimal(position.exit_price, 'margin position exit_price', {
    allowNegative: false,
    allowZero: false,
  })
  const limitPriceText = nullableTradingDecimal(position.limit_price, 'margin position limit_price', {
    allowNegative: false,
    allowZero: false,
  })
  const realizedPnlText = nullableTradingDecimal(position.realized_pnl, 'margin position realized_pnl')
  const interestAmountText = nonNegativeTradingDecimal(position.interest_amount, 'margin position interest_amount')
  return {
    id: String(position.id),
    productId: asNumber(position.product_id),
    pairId: asNumber(position.pair_id),
    marginAssetId: asNumber(position.margin_asset),
    direction: String(position.direction || '').toLowerCase() === 'short' ? 'short' : 'long',
    marginMode: String(position.margin_mode || 'isolated').toLowerCase() === 'cross' ? 'cross' : 'isolated',
    marginAmount: decimalDisplayNumber(marginAmountText, 'margin position margin_amount'),
    notionalAmount: decimalDisplayNumber(notionalAmountText, 'margin position notional_amount'),
    borrowedAmount: decimalDisplayNumber(borrowedAmountText, 'margin position borrowed_amount'),
    marginAmountText,
    notionalAmountText,
    borrowedAmountText,
    leverage: asNumber(position.leverage, 1),
    orderType: String(position.order_type || '').trim().toLowerCase() === 'limit' ? 'limit' : 'market',
    entryPrice: nullableDecimalDisplayNumber(entryPriceText),
    entryPriceText,
    exitPrice: nullableDecimalDisplayNumber(exitPriceText),
    exitPriceText,
    limitPrice: limitPriceText,
    limitPriceText,
    realizedPnl: realizedPnlText ? decimalDisplayNumber(realizedPnlText, 'margin position realized_pnl') : Number.NaN,
    interestAmount: decimalDisplayNumber(interestAmountText, 'margin position interest_amount'),
    realizedPnlText,
    interestAmountText,
    status: String(position.status || 'open'),
    openedAt: normalizeTimestamp(position.opened_at, 'margin position opened_at'),
    createdAt: normalizeTimestamp(position.created_at, 'margin position created_at'),
    closedAt: normalizeTimestamp(position.closed_at, 'margin position closed_at'),
  }
}

export function mapMarginPositionExecution(execution: Record<string, unknown>): MarginPositionExecution {
  const id = String(execution.id ?? '').trim()
  const positionId = String(execution.position_id ?? '').trim()
  const idempotencyKey = String(execution.idempotency_key ?? '').trim()
  const closePercentage = typeof execution.close_percentage === 'number'
    ? execution.close_percentage
    : Number(execution.close_percentage)
  if (!id || !positionId || !idempotencyKey
    || !Number.isSafeInteger(closePercentage)
    || closePercentage < 1
    || closePercentage > 100
    || typeof execution.fully_closed !== 'boolean') {
    throw new TradingFinancialContractError('margin position execution identity')
  }
  return {
    id,
    positionId,
    idempotencyKey,
    closePercentage,
    closeMarginAmountText: nonNegativeTradingDecimal(execution.close_margin_amount, 'margin execution close_margin_amount'),
    closeNotionalAmountText: nonNegativeTradingDecimal(execution.close_notional_amount, 'margin execution close_notional_amount'),
    closeBorrowedAmountText: nonNegativeTradingDecimal(execution.close_borrowed_amount, 'margin execution close_borrowed_amount'),
    closeInterestAmountText: nonNegativeTradingDecimal(execution.close_interest_amount, 'margin execution close_interest_amount'),
    exitPriceText: tradingDecimal(execution.exit_price, 'margin execution exit_price', {
      allowNegative: false,
      allowZero: false,
    }),
    realizedPnlText: tradingDecimal(execution.realized_pnl, 'margin execution realized_pnl'),
    settlementAmountText: tradingDecimal(execution.settlement_amount, 'margin execution settlement_amount'),
    fullyClosed: execution.fully_closed,
    createdAt: normalizeTimestamp(execution.created_at, 'margin execution created_at')!,
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

function nonNegativeInteger(value: unknown): number | null {
  const parsed = typeof value === 'string' && value.trim() ? Number(value) : value
  return typeof parsed === 'number' && Number.isInteger(parsed) && parsed >= 0 ? parsed : null
}

function uniqueSpotOrders(orders: SpotOrder[]): SpotOrder[] {
  return [...new Map(orders.map((order) => [order.id, order])).values()]
    .sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0))
}

function mapMarginWallet(wallet: Record<string, unknown>): MarginWalletAccount {
  const availableText = nonNegativeTradingDecimal(wallet.available, 'margin wallet available')
  const frozenText = nonNegativeTradingDecimal(wallet.frozen, 'margin wallet frozen')
  const lockedText = nonNegativeTradingDecimal(wallet.locked, 'margin wallet locked')
  const maxTransferableToSpotText = nonNegativeTradingDecimal(
    wallet.max_transferable_to_spot,
    'margin wallet max_transferable_to_spot',
  )
  const transferRiskEquityText = nullableTradingDecimal(
    wallet.transfer_risk_equity,
    'margin wallet transfer_risk_equity',
  )
  const transferRiskMaintenanceMarginText = nullableTradingDecimal(
    wallet.transfer_risk_maintenance_margin,
    'margin wallet transfer_risk_maintenance_margin',
    { allowNegative: false },
  )
  return {
    assetId: asNumber(wallet.asset_id),
    symbol: String(wallet.asset_symbol || '').toUpperCase(),
    logoUrl: String(wallet.logo_url || '').trim() || undefined,
    marginTransferEnabled: wallet.margin_transfer_enabled !== false,
    available: decimalDisplayNumber(availableText, 'margin wallet available'),
    frozen: decimalDisplayNumber(frozenText, 'margin wallet frozen'),
    locked: decimalDisplayNumber(lockedText, 'margin wallet locked'),
    availableText,
    frozenText,
    lockedText,
    maxTransferableToSpot: decimalDisplayNumber(maxTransferableToSpotText, 'margin wallet max_transferable_to_spot'),
    maxTransferableToSpotText,
    transferRiskEquity: nullableDecimalDisplayNumber(transferRiskEquityText),
    transferRiskEquityText,
    transferRiskMaintenanceMargin: nullableDecimalDisplayNumber(transferRiskMaintenanceMarginText),
    transferRiskMaintenanceMarginText,
  }
}

function mapMarginCrossAccount(account: Record<string, unknown>): MarginCrossAccount {
  const equityText = tradingDecimal(account.equity, 'margin cross account equity')
  const unrealizedPnlText = tradingDecimal(account.unrealized_pnl, 'margin cross account unrealized_pnl')
  const interestAmountText = nonNegativeTradingDecimal(account.interest_amount, 'margin cross account interest_amount')
  const maintenanceMarginText = nonNegativeTradingDecimal(account.maintenance_margin, 'margin cross account maintenance_margin')
  const marginRatioText = nullableTradingDecimal(account.margin_ratio, 'margin cross account margin_ratio')
  return {
    marginAssetId: asNumber(account.margin_asset),
    status: String(account.status || ''),
    equity: decimalDisplayNumber(equityText, 'margin cross account equity'),
    unrealizedPnl: decimalDisplayNumber(unrealizedPnlText, 'margin cross account unrealized_pnl'),
    interestAmount: decimalDisplayNumber(interestAmountText, 'margin cross account interest_amount'),
    maintenanceMargin: decimalDisplayNumber(maintenanceMarginText, 'margin cross account maintenance_margin'),
    marginRatio: nullableDecimalDisplayNumber(marginRatioText),
    equityText,
    unrealizedPnlText,
    interestAmountText,
    maintenanceMarginText,
    marginRatioText,
  }
}

function mapStrictMarginCrossAccountRisk(value: unknown): MarginCrossAccountRiskFinancialAuthority | undefined {
  if (value === null || value === undefined) return undefined
  if (typeof value !== 'object' || Array.isArray(value)) {
    throw new TradingFinancialContractError('cross account risk')
  }
  const risk = value as Record<string, unknown>
  const equityText = tradingDecimal(risk.equity, 'cross account risk equity')
  const maintenanceMarginText = nonNegativeTradingDecimal(risk.maintenance_margin, 'cross account risk maintenance_margin')
  const liquidationBufferText = tradingDecimal(risk.liquidation_buffer, 'cross account risk liquidation_buffer')
  const marginRatioText = nullableTradingDecimal(risk.margin_ratio, 'cross account risk margin_ratio')
  const unrealizedPnlText = tradingDecimal(risk.unrealized_pnl, 'cross account risk unrealized_pnl')
  const interestAmountText = nonNegativeTradingDecimal(risk.interest_amount, 'cross account risk interest_amount')
  const netQuantityText = tradingDecimal(risk.net_quantity, 'cross account risk net_quantity')
  const grossQuantityText = nonNegativeTradingDecimal(risk.gross_quantity, 'cross account risk gross_quantity')
  const conditionalLiquidationPriceText = nullableTradingDecimal(
    risk.conditional_liquidation_price,
    'cross account risk conditional_liquidation_price',
    { allowNegative: false, allowZero: false },
  )
  const conditionalLiquidationDistanceRateText = nullableTradingDecimal(
    risk.conditional_liquidation_distance_rate,
    'cross account risk conditional_liquidation_distance_rate',
    { allowNegative: false },
  )
  const mapped = mapMarginCrossAccountRisk(risk)
  if (!mapped) throw new TradingFinancialContractError('cross account risk')
  return {
    ...mapped,
    equityText,
    maintenanceMarginText,
    liquidationBufferText,
    marginRatioText,
    unrealizedPnlText,
    interestAmountText,
    netQuantityText,
    grossQuantityText,
    conditionalLiquidationPriceText,
    conditionalLiquidationDistanceRateText,
  }
}

function tradingDecimal(
  value: unknown,
  field: string,
  constraints: { allowNegative?: boolean; allowZero?: boolean } = {},
): DecimalText {
  try {
    return requiredDecimalText(value, field, 'trading financial response', {
      ...FINANCIAL_DECIMAL_CONSTRAINTS,
      ...constraints,
    })
  } catch {
    throw new TradingFinancialContractError(field)
  }
}

function nonNegativeTradingDecimal(value: unknown, field: string): DecimalText {
  return tradingDecimal(value, field, { allowNegative: false })
}

function nullableTradingDecimal(
  value: unknown,
  field: string,
  constraints: { allowNegative?: boolean; allowZero?: boolean } = {},
): DecimalText | null {
  if (value === null || value === undefined) return null
  return tradingDecimal(value, field, constraints)
}

function decimalDisplayNumber(value: DecimalText, field: string): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) throw new TradingFinancialContractError(field)
  return parsed
}

function nullableDecimalDisplayNumber(value: DecimalText | null): number | null {
  return value ? decimalDisplayNumber(value, 'decimal display value') : null
}
