export interface MarketTicker {
  id?: number
  symbol: string
  base: string
  quote: string
  iconUrl?: string
  baseIconUrl?: string
  quoteIconUrl?: string
  lastPrice: number
  openPrice: number
  highPrice: number
  lowPrice: number
  volume: number
  changePercent: number
  observedAt?: number
}

export interface MarketFavorite {
  marketId: number
  symbol: string
  iconUrl?: string
  baseIconUrl?: string
  quoteIconUrl?: string
}

export interface MarketPair {
  id: number
  symbol: string
  base: string
  quote: string
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
  name?: string
  depositEnabled: boolean
  minDepositAmount: number
  logoUrl?: string
}

export interface DepositNetwork {
  network: string
  displayName: string
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
  marginTransferEnabled?: boolean
  available: number
  frozen: number
  locked: number
}

export type MarginOrderType = 'market' | 'limit'

export interface MarginProduct {
  id: number
  pairId: number
  symbol: string
  marginAssetId: number
  marginAssetSymbol: string
  logoUrl?: string
  marginMode: 'cross' | 'isolated'
  marginModes: Array<'cross' | 'isolated'>
  orderTypes: MarginOrderType[]
  pricePrecision: number | null
  leverageLevels: number[]
  maxLeverage: number
  minMargin: number
  maxMargin: number | null
  maintenanceMarginRate: number
  hourlyInterestRate: number
  takeProfitStopLossSupported: boolean
  strategyOrdersSupported: boolean
  bulkCloseSupported: boolean
  positionRiskSupported: boolean
}

export interface NewsItem {
  id: number
  title: string
  category?: string
  bannerUrl?: string
  publishedAt?: number
}
