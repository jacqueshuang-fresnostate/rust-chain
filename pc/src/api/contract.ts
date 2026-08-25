import request from './request'
import { fetchHistoryKLine as fetchMarketKLine, fetchLatestTrade, fetchMarketSnapshot, fetchTradePlate } from './market'
import {
  backendApiUrl,
  mapMarginWalletsToContractWallets,
  mapMarginPositionsToContractOrders,
  mapMarginProductsToContractCoins,
  mapPcMarginOpenRequest,
  type BackendMarginWalletsResponse,
  type BackendMarginPositionsResponse,
  type BackendMarginProductsResponse,
  type BackendMarginBatchActionResponse,
} from './backendAdapters'
import type { MarginBatchActionResult, MarginMode } from '../domain/marginActions'

export type OpenDirection = 0 | 1
export type OrderType = 0 | 1 | 2

export interface OpenPositionParams {
  contractCoinId: number
  direction: OpenDirection
  type: OrderType
  triggerPrice?: number
  entrustPrice?: number
  leverage: number
  marginMode?: MarginMode
  volume: number
}

export interface ClosePositionParams {
  contractCoinId: number
  positionId: string
}

export interface OrderListParams {
  contractCoinId?: number
  symbol?: string
  status?: number
  pageNo: number
  pageSize: number
}

export interface TransferParams {
  unit: string
  from: 'SPOT' | 'SWAP'
  to: 'SPOT' | 'SWAP'
  fromWalletId?: number
  toWalletId?: number
  amount: number
}

export interface MarginActionResult<T = unknown> {
  code: number
  message: string
  data: T
}

export async function fetchBaseSymbol(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginProductsResponse>(backendApiUrl('/margin/products'))
  return { data: [...new Set(response.data.products.map((product) => product.margin_asset_symbol))] }
}

export async function fetchCoinInfo(symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginProductsResponse>(backendApiUrl('/margin/products'))
  const mapped = mapMarginProductsToContractCoins(response.data).data as any[]
  return { data: mapped.find((item) => normalizeSymbol(item.symbol) === normalizeSymbol(symbol)) ?? null }
}

export async function openPosition(params: OpenPositionParams): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl('/margin/positions'), mapPcMarginOpenRequest(params, createMarginIdempotencyKey()))
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export async function closePosition(params: ClosePositionParams): Promise<{ data: any }> {
  const response = await request.instance.post(
    backendApiUrl(`/margin/positions/${encodeURIComponent(params.positionId)}/close`),
    {},
  )
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export async function closeAllPositions(contractCoinId: number): Promise<{ data: MarginActionResult<MarginBatchActionResult> }> {
  const response = await request.instance.post<BackendMarginBatchActionResponse>(
    backendApiUrl('/margin/positions/close-all'),
    { product_id: contractCoinId || undefined },
  )
  return { data: { code: 0, message: 'success', data: mapMarginBatchAction(response.data) } }
}

export async function cancelOrder(entrustId: string): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl(`/margin/positions/${encodeURIComponent(entrustId)}/cancel`), {})
  return { data: { code: 0, message: 'success', data: response.data } }
}

export function cancelAllOrders(symbol?: string): Promise<{ data: any }> {
  return request.instance
    .post(backendApiUrl('/margin/positions/cancel-all'), productIdPayload(symbol))
    .then((response) => ({ data: { code: 0, message: 'success', data: response.data } }))
}

export async function fetchOrderDetail(orderId: string): Promise<{ data: any }> {
  const response = await request.instance.get(backendApiUrl(`/margin/positions/${encodeURIComponent(orderId)}`))
  return { data: response.data }
}

export async function fetchCurrentOrders(params: OrderListParams): Promise<{ data: any }> {
  return fetchMarginOrders({ ...params, status: 0 })
}

export async function fetchHistoryOrders(params: OrderListParams): Promise<{ data: any }> {
  return fetchMarginOrders({ ...params, status: 1 })
}

export async function fetchContractWallets(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginWalletsResponse>(backendApiUrl('/margin/wallets'))
  return { data: mapMarginWalletsToContractWallets(response.data) }
}

export async function fetchWalletDetail(contractCoinId: number): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginWalletsResponse>(backendApiUrl('/margin/wallets'))
  const wallet = (mapMarginWalletsToContractWallets(response.data).data as any[]).find((item) => item.id === contractCoinId)
  return { data: { code: 0, message: 'success', data: wallet ?? null } }
}

export async function transferFunds(params: TransferParams): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl('/margin/transfers'), {
    asset_symbol: params.unit,
    from: params.from === 'SWAP' ? 'margin' : 'spot',
    to: params.to === 'SWAP' ? 'margin' : 'spot',
    amount: String(params.amount),
  })
  return { data: { code: 0, message: 'success', data: response.data } }
}

export interface MarginUserSetting {
  leverage: number | null
  marginMode: MarginMode | null
}

/// 读取服务端已保存的杠杆与仓位模式，避免刷新后表单回退到本地默认值。
export async function fetchMarginSetting(contractCoinId: number): Promise<MarginUserSetting> {
  const response = await request.instance.get<{ leverage?: string | null; margin_mode?: string | null }>(
    backendApiUrl(`/margin/settings/${contractCoinId}`),
  )
  const leverage = Number(response.data.leverage)
  return {
    leverage: Number.isFinite(leverage) && leverage > 0 ? leverage : null,
    marginMode: response.data.margin_mode === 'cross' || response.data.margin_mode === 'isolated'
      ? response.data.margin_mode
      : null,
  }
}

export async function modifyLeverage(contractCoinId: number, leverage: number, _direction: 0 | 1): Promise<{ data: any }> {
  const response = await request.instance.patch(backendApiUrl(`/margin/settings/${contractCoinId}/leverage`), {
    leverage: String(leverage),
  })
  return { data: { code: 0, message: 'success', data: response.data } }
}

export function canSwitchPattern(_contractCoinId: number, _targetPattern: string): Promise<{ data: any }> {
  return Promise.resolve({ data: { code: 0, message: 'success', data: true } })
}

export async function switchPattern(contractCoinId: number, targetPattern: MarginMode): Promise<{ data: any }> {
  const response = await request.instance.patch(backendApiUrl(`/margin/settings/${contractCoinId}/mode`), {
    margin_mode: targetPattern,
  })
  return { data: { code: 0, message: 'success', data: response.data } }
}

export async function fetchContractSymbols(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginProductsResponse>(backendApiUrl('/margin/products'))
  return { data: mapMarginProductsToContractCoins(response.data).data }
}

export async function fetchSymbolThumb(): Promise<{ data: any }> {
  const response = await fetchMarketSnapshot()
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data.map((item: any) => ({
        symbol: item.symbol,
        open: item.open,
        close: item.close,
        last: item.close,
        high: item.high,
        low: item.low,
        vol: item.volume,
        change: item.chg,
      })),
    },
  }
}

export { fetchLatestTrade }

export function fetchSymbolInfo(symbol: string): Promise<{ data: any }> {
  return fetchCoinInfo(symbol)
}

export async function fetchExchangePlate(symbol: string): Promise<{ data: any }> {
  return fetchTradePlate(symbol)
}

export function fetchKlineHistory(symbol: string, from: number, to: number, resolution: string): Promise<{ data: any }> {
  return fetchMarketKLine(symbol, resolution, from, to)
}

async function fetchMarginOrders(params: OrderListParams): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginPositionsResponse>(backendApiUrl('/margin/positions'))
  const mapped = mapMarginPositionsToContractOrders(response.data).data as any[]
  const filtered = mapped.filter((item) => {
    const productMatches = !params.contractCoinId || item.orderId === String(params.contractCoinId) || item.productId === params.contractCoinId
    const symbolMatches = !params.symbol || normalizeSymbol(item.symbol) === normalizeSymbol(params.symbol)
    const statusMatches = params.status === undefined || item.status === params.status
    return productMatches && symbolMatches && statusMatches
  })
  return { data: { code: 0, message: 'success', data: filtered } }
}

export function mapMarginBatchAction(response: BackendMarginBatchActionResponse): MarginBatchActionResult {
  return {
    succeeded: response.positions.map((position) => String(position.id)),
    failures: response.failures.map((failure) => ({
      id: String(failure.id),
      code: failure.code,
      message: failure.message,
    })),
  }
}

function normalizeSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function createMarginIdempotencyKey(): string {
  return `pc-margin-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}

function productIdPayload(symbol?: string): { product_id?: number } {
  const id = Number(symbol)
  return Number.isFinite(id) && id > 0 ? { product_id: id } : {}
}
