export interface MarketTicker {
  id?: number
  symbol: string
  base: string
  quote: string
  iconUrl?: string
  lastPrice: number
  openPrice: number
  highPrice: number
  lowPrice: number
  volume: number
  changePercent: number
  observedAt?: number
}

export interface KlinePoint {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export interface OrderBookLevel {
  price: number
  quantity: number
}

export interface TradePrint {
  id: string
  side: 'buy' | 'sell'
  price: number
  quantity: number
  time: number
}

export interface DepositAsset {
  symbol: string
  depositEnabled: boolean
  minDepositAmount: number
  logoUrl?: string
}

export interface DepositNetwork {
  network: string
  displayName: string
  estimatedMinutes: number
  minDepositAmount: number
}

export interface DepositAddress {
  assetSymbol: string
  network: string
  address: string
  memo?: string
  minDepositAmount: number
}

export interface WalletAccount {
  assetId: number
  symbol: string
  logoUrl?: string
  available: number
  frozen: number
  locked: number
}

export interface MarginProduct {
  id: number
  symbol: string
  marginAssetSymbol: string
  marginMode: 'cross' | 'isolated'
  marginModes: Array<'cross' | 'isolated'>
  leverageLevels: number[]
  maxLeverage: number
  minMargin: number
}

export interface NewsItem {
  id: number
  title: string
  publishedAt?: number
}
