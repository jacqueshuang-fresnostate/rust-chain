import request from './request'
import {
  backendApiUrl,
  mapMarketDepthToTradePlate,
  mapMarketKlinesToPcRows,
  mapMarketTradeToPcTrade,
  mapMarketsToPcTickers,
  type BackendMarket,
  type BackendMarketListResponse,
  type BackendMarketTicker,
} from './backendAdapters'

export interface MarketSnapshot {
  symbol: string
  open: number
  high: number
  low: number
  close: number
  volume: number
  turnover: number
  time: number
  chg: number
  zone: number
}

export async function fetchMarketSnapshot(): Promise<{ data: any }> {
  const marketsResponse = await request.instance.get<BackendMarketListResponse>(backendApiUrl('/markets'))
  const tickersBySymbol = await fetchTickersBySymbol(marketsResponse.data.markets)
  return { data: mapMarketsToPcTickers(marketsResponse.data, tickersBySymbol) }
}

export async function fetchHistoryKLine(symbol: string, resolution: string, from: number, to: number): Promise<{ data: any }> {
  const response = await request.instance.get(backendApiUrl(`/markets/${marketPathSymbol(symbol)}/klines`), {
    params: {
      interval: normalizeKlineInterval(resolution),
      start: normalizeTimestamp(from),
      end: normalizeTimestamp(to),
      limit: 100,
    },
  })
  return { data: mapMarketKlinesToPcRows(response.data) }
}

export async function fetchTradePlate(symbol: string): Promise<{ data: any }> {
  const response = await request.instance.get(backendApiUrl(`/markets/${marketPathSymbol(symbol)}/depth`))
  return { data: mapMarketDepthToTradePlate(response.data) }
}

export async function fetchLatestTrade(symbol: string, size: number = 20): Promise<{ data: any }> {
  const response = await request.instance.get(backendApiUrl(`/markets/${marketPathSymbol(symbol)}/trades`), {
    params: { limit: size },
  })
  return { data: (response.data.trades || []).map(mapMarketTradeToPcTrade) }
}

async function fetchTickersBySymbol(markets: BackendMarket[]): Promise<Record<string, BackendMarketTicker>> {
  const results = await Promise.allSettled(
    markets.map((market) => request.instance.get<BackendMarketTicker>(backendApiUrl(`/markets/${marketPathSymbol(market.symbol)}/ticker`))),
  )
  const tickers: Record<string, BackendMarketTicker> = {}

  results.forEach((result) => {
    if (result.status !== 'fulfilled') return
    const ticker = result.value.data
    tickers[compactMarketSymbol(ticker.symbol)] = ticker
  })

  return tickers
}

function marketPathSymbol(symbol: string): string {
  return encodeURIComponent(compactMarketSymbol(symbol))
}

function compactMarketSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function normalizeKlineInterval(resolution: string): string {
  const normalized = resolution.trim().toLowerCase()
  if (normalized.endsWith('min')) return `${Number.parseInt(normalized, 10) || 1}m`
  if (normalized === '1day') return '1d'
  return normalized
}

function normalizeTimestamp(value: number): number {
  return value > 0 && value < 1_000_000_000_000 ? value * 1000 : value
}
