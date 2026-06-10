import request from './request'
import { fetchHistoryKLine as fetchMarketKLine, fetchLatestTrade, fetchMarketSnapshot, fetchTradePlate } from './market'
import {
  backendApiUrl,
  mapMarginPositionsToContractOrders,
  mapMarginPositionsToContractWallets,
  mapMarginProductsToContractCoins,
  mapPcMarginOpenRequest,
  type BackendMarginPositionsResponse,
  type BackendMarginProductsResponse,
} from './backendAdapters'

export type OpenDirection = 0 | 1
export type CloseDirection = 0 | 1
export type OrderType = 0 | 1 | 2

export interface OpenPositionParams {
  contractCoinId: number
  direction: OpenDirection
  type: OrderType
  triggerPrice?: number
  entrustPrice?: number
  leverage: number
  volume: number
}

export interface ClosePositionParams {
  contractCoinId: number
  direction: CloseDirection
  type: OrderType
  triggerPrice?: number
  entrustPrice?: number
  volume: number
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
  const positionId = await resolveOpenPositionId(params)
  const response = await request.instance.post(backendApiUrl(`/margin/positions/${positionId}/close`), {})
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export function closeAllPositions(_contractCoinId: number, _type: 0 | 1 | 2): Promise<{ data: any }> {
  return Promise.reject(new Error('Closing all margin positions is not supported by the Rust backend yet.'))
}

export function cancelOrder(_entrustId: string): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin order cancellation is not supported by the Rust backend yet.'))
}

export function cancelAllOrders(_symbol?: string): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin order cancellation is not supported by the Rust backend yet.'))
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
  const response = await request.instance.get<BackendMarginPositionsResponse>(backendApiUrl('/margin/positions'))
  return { data: mapMarginPositionsToContractWallets(response.data) }
}

export async function fetchWalletDetail(contractCoinId: number): Promise<{ data: any }> {
  const response = await request.instance.get<BackendMarginPositionsResponse>(backendApiUrl('/margin/positions'))
  const wallet = (mapMarginPositionsToContractWallets(response.data).data as any[]).find((item) => item.id === contractCoinId)
  return { data: { code: 0, message: 'success', data: wallet ?? null } }
}

export function transferFunds(_params: TransferParams): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin wallet transfer is not supported by the Rust backend yet.'))
}

export function modifyLeverage(_contractCoinId: number, _leverage: number, _direction: 0 | 1): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin leverage modification is not supported by the Rust backend yet.'))
}

export function canSwitchPattern(_contractCoinId: number, _targetPattern: string): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin mode switching is not supported by the Rust backend yet.'))
}

export function switchPattern(_contractCoinId: number, _targetPattern: string): Promise<{ data: any }> {
  return Promise.reject(new Error('Margin mode switching is not supported by the Rust backend yet.'))
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

async function resolveOpenPositionId(params: ClosePositionParams): Promise<string> {
  const response = await request.instance.get<BackendMarginPositionsResponse>(backendApiUrl('/margin/positions'), {
    params: { status: 'opened' },
  })
  const position = response.data.positions.find((item) => {
    return item.product_id === params.contractCoinId && (params.direction === 1 ? item.direction !== 'short' : item.direction === 'short')
  })
  if (!position) throw new Error('No open margin position is available to close.')
  return String(position.id)
}

function normalizeSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function createMarginIdempotencyKey(): string {
  return `pc-margin-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
