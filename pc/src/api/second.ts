import request from './request'
import { fetchHistoryKLine, fetchLatestTrade, fetchTradePlate } from './market'
import {
  backendApiUrl,
  mapPcSecondsOrderRequest,
  mapSecondsOrdersToPcOrders,
  mapSecondsProductsToPcCycles,
  mapSecondsProductsToPcTickers,
  type BackendMarketTicker,
  type BackendSecondsOrdersResponse,
  type BackendSecondsProductsResponse,
  type BackendWalletAccountsResponse,
} from './backendAdapters'

export const fetchSecondLatestTrade = fetchLatestTrade
export const fetchSecondExchangePlate = fetchTradePlate
export const fetchSecondKlineHistory = fetchHistoryKLine

export { fetchHistoryKLine, fetchLatestTrade }

export interface SecondOrderParams {
  symbol: string
  coinSymbol: string
  direction: 0 | 1
  cycleId: number
  productId?: number
  durationSeconds?: number
  amount: number
}

export interface SecondHistoryParams {
  symbol?: string
  pageNo: number
  pageSize: number
}

export interface SecondTransferParams {
  unit: string
  from: 'SPOT' | 'SECOND'
  to: 'SPOT' | 'SECOND'
  amount: number
}

export async function fetchSecondSnapshot(): Promise<{ data: any }> {
  const productsResponse = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  const tickersBySymbol = await fetchSecondTickersBySymbol(productsResponse.data.products)
  return { data: mapSecondsProductsToPcTickers(productsResponse.data, tickersBySymbol) }
}

export async function fetchSecondSymbols(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  return { data: [...new Set(response.data.products.filter(isActiveSecondsProduct).map((product) => displaySecondSymbol(product.symbol)))] }
}

export async function fetchSecondSymbolInfo(symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  return { data: response.data.products.find((product) => normalizeSymbol(product.symbol) === normalizeSymbol(symbol)) ?? null }
}

export async function submitSecondOrder(params: SecondOrderParams): Promise<{ data: any }> {
  const response = await request.instance.post(backendApiUrl('/seconds-contracts/orders'), mapPcSecondsOrderRequest(params, createSecondsIdempotencyKey()))
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data,
    },
  }
}

export async function fetchSecondCurrentOrders(symbol: string): Promise<{ data: any }> {
  return fetchSecondOrders({ symbol, status: 'open' })
}

export async function fetchSecondOrderResult(id: number, symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsOrdersResponse>(backendApiUrl('/seconds-contracts/orders'))
  const order = mapSecondsOrdersToPcOrders({ orders: response.data.orders.filter((item) => item.id === id && symbolMatches(item.symbol, symbol)) }).data[0] ?? null
  return { data: { code: 0, message: 'success', data: order } }
}

export async function fetchSecondHistoryOrders(params: SecondHistoryParams): Promise<{ data: any }> {
  return fetchSecondOrders({ symbol: params.symbol, status: 'closed' })
}

export async function fetchSecondCycles(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsProductsResponse>(backendApiUrl('/seconds-contracts/products'))
  return { data: mapSecondsProductsToPcCycles(response.data) }
}

export async function fetchSecondCoins(): Promise<{ data: any }> {
  return fetchSecondSymbols()
}

export async function fetchSecondWallets(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  return { data: response.data.accounts }
}

export function transferSecondFunds(_params: SecondTransferParams): Promise<{ data: any }> {
  return Promise.reject(new Error('Seconds contract transfer is not supported by the Rust backend yet.'))
}

export async function fetchSecondBalance(symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  const normalized = normalizeAssetSymbol(symbol)
  const account = response.data.accounts.find((item) => normalizeAssetSymbol(item.symbol) === normalized)
  return { data: { code: 0, message: 'success', data: Number(account?.available ?? 0) || 0 } }
}

async function fetchSecondOrders(params: { symbol?: string; status?: 'open' | 'closed' }): Promise<{ data: any }> {
  const response = await request.instance.get<BackendSecondsOrdersResponse>(backendApiUrl('/seconds-contracts/orders'))
  const orders = response.data.orders.filter((order) => {
    const statusMatches = !params.status || (params.status === 'open' ? isOpenStatus(order.status) : !isOpenStatus(order.status))
    return statusMatches && symbolMatches(order.symbol, params.symbol)
  })
  return { data: mapSecondsOrdersToPcOrders({ orders }) }
}

async function fetchSecondTickersBySymbol(products: BackendSecondsProductsResponse['products']): Promise<Record<string, BackendMarketTicker>> {
  const symbols = [
    ...new Set(
      products
        .filter(isActiveSecondsProduct)
        .map((product) => normalizeSymbol(product.symbol)),
    ),
  ]
  const results = await Promise.allSettled(
    symbols.map((symbol) => request.instance.get<BackendMarketTicker>(backendApiUrl(`/markets/${encodeURIComponent(symbol)}/ticker`))),
  )
  const tickers: Record<string, BackendMarketTicker> = {}

  results.forEach((result) => {
    if (result.status !== 'fulfilled') return
    const ticker = result.value.data
    tickers[normalizeSymbol(ticker.symbol)] = ticker
  })

  return tickers
}

function isActiveSecondsProduct(product: { status?: string }): boolean {
  return String(product.status || 'active').toLowerCase() === 'active'
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

function displaySecondSymbol(symbol: string): string {
  const normalized = symbol.toUpperCase()
  if (normalized.includes('/')) return normalized
  if (normalized.includes('-')) return normalized.replace('-', '/')
  if (normalized.endsWith('USDT') && normalized.length > 4) return `${normalized.slice(0, -4)}/USDT`
  return normalized.replace('_', '/')
}

function normalizeAssetSymbol(symbol: string): string {
  return symbol.split('-')[0].toUpperCase()
}

function createSecondsIdempotencyKey(): string {
  return `pc-seconds-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
