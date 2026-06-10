import { APP_CONFIG } from '../config/app.ts'

export interface BackendAuthTokenResponse {
  access_token: string
  refresh_token: string
  token_type: string
  scope: string
}

export interface PcAuthResponse {
  code: number
  message: string
  data: {
    token: string
    accessToken: string
    refreshToken: string
    tokenType: string
    scope: string
  }
}

export interface BackendUserProfile {
  id: number
  email?: string | null
  phone?: string | null
  status: string
  kyc_level: number
  email_verified_at?: number | null
  fund_password_set: boolean
  created_at: number
}

export interface PcSecurityProfile {
  username?: string
  id: number
  createTime: string
  email?: string
  phone?: string
  fundsVerified: number
  emailVerified: number
  phoneVerified: number
  realVerified: number
  realAuditing: number
  transactionStatus: number
}

export interface BackendWalletAccountsResponse {
  accounts: BackendWalletAccount[]
}

export interface BackendWalletAccount {
  user_id: number
  asset_id: number
  symbol: string
  available: string | number
  frozen: string | number
  locked: string | number
}

export interface PcCoin {
  name: string
  unit: string
  coinGroup: string
  canWithdraw: number | boolean
  canRecharge: number | boolean
}

export interface PcMemberWallet {
  id: string | number
  memberId: string | number
  coin: PcCoin
  balance: number
  frozenBalance: number
  address: string
}

export interface BackendWalletLedgerResponse {
  entries: BackendWalletLedgerEntry[]
}

export interface BackendWalletLedgerEntry {
  id: number
  user_id: number
  asset_id: number
  symbol: string
  change_type: string
  amount: string | number
  balance_type: string
  balance_after: string | number
  available_after: string | number
  frozen_after: string | number
  locked_after: string | number
  ref_type: string
  ref_id: string
  created_at: number
}

export interface PcApiResponse<T> {
  code: number
  message: string
  data: T
}

export interface PcPageData<T> {
  content: T[]
  page: {
    number: number
    size: number
    totalElements: number
    totalPages: number
  }
}

export interface PcTransactionRecord {
  id: number
  memberId: number
  amount: number
  fee: number
  symbol: string
  type: number
  createTime: string
  address?: string
  status: number
}

export interface BackendMarketListResponse {
  markets: BackendMarket[]
}

export interface BackendMarket {
  id?: number
  symbol: string
  base_asset?: string
  quote_asset?: string
  price_precision?: number
  qty_precision?: number
  min_order_value?: string | number
  status?: string
  market_type?: string
}

export interface BackendMarketTicker {
  symbol: string
  last_price: string | number
  volume_24h: string | number
  observed_at: number
}

export interface PcMarketTicker {
  symbol: string
  icon: string
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

export interface BackendMarketKline {
  symbol: string
  interval: string
  open_time: number
  open: string | number
  high: string | number
  low: string | number
  close: string | number
  volume: string | number
}

export interface BackendMarketDepthLevel {
  price: string | number
  amount?: string | number
  quantity?: string | number
}

export interface BackendMarketDepth {
  symbol: string
  bids: BackendMarketDepthLevel[]
  asks: BackendMarketDepthLevel[]
  observed_at?: number
}

export interface PcMarketDepthLevel {
  price: number
  amount: number
}

export interface PcMarketDepth {
  symbol?: string
  bids: PcMarketDepthLevel[]
  asks: PcMarketDepthLevel[]
  time?: number
}

export interface BackendMarketTrade {
  id?: string | number
  symbol?: string
  side?: string
  direction?: string
  price: string | number
  quantity?: string | number
  amount?: string | number
  traded_at?: number
  time?: number
}

export interface PcMarketTrade {
  id?: string | number
  symbol?: string
  direction: 'BUY' | 'SELL'
  price: number
  amount: number
  time: number
}

export interface BackendSpotOrder {
  id: string
  user_id: string
  pair_id: string
  side: 'buy' | 'sell'
  order_type: 'limit' | 'market'
  price?: string | number | null
  quantity: string | number
  filled_quantity: string | number
  status: string
}

export interface BackendSpotOrdersResponse {
  orders: BackendSpotOrder[]
}

export interface PcSpotOrderRow {
  orderId: string
  symbol: string
  direction: 'BUY' | 'SELL'
  type: 'LIMIT_PRICE' | 'MARKET_PRICE'
  price: number
  amount: number
  filledAmount: number
  status: string
  time: number
}

export interface PcSpotOrderParams {
  symbol: string
  price?: number
  amount: number
  direction: 'BUY' | 'SELL'
  type: 'LIMIT_PRICE' | 'MARKET_PRICE' | 'STOP_LIMIT'
}

export interface BackendCreateSpotOrderRequest {
  pair_id: string
  side: 'buy' | 'sell'
  order_type: 'limit' | 'market'
  price?: string
  quantity: string
  reference_price?: string
  idempotency_key: string
}

export interface PcTradeWalletBalance {
  symbol: string
  balance: number
  frozenBalance: number
}

export interface BackendConvertPair {
  id: number
  from_asset_id: number
  from_asset_symbol?: string
  to_asset_id: number
  to_asset_symbol?: string
  pricing_mode: string
  spread_rate: string | number
  min_amount: string | number
  max_amount?: string | number | null
  enabled: boolean
}

export interface BackendConvertPairsResponse {
  pairs: BackendConvertPair[]
}

export interface PcSwapCoinRow {
  id: number
  fromUnit: string
  toUnit: string
  minAmount: number
  maxAmount: number
  enabled: boolean
}

export interface BackendEarnProduct {
  id: number
  asset_id: number
  asset_symbol: string
  name: string
  category: string
  introduction_json: unknown
  term_days: number
  apr_rate: string | number
  min_subscribe: string | number
  max_subscribe?: string | number | null
  status: string
}

export interface BackendEarnProductsResponse {
  products: BackendEarnProduct[]
}

export interface BackendEarnSubscription {
  id: number
  user_id: number
  product_id: number
  asset_id: number
  asset_symbol?: string
  amount: string | number
  apr_rate: string | number
  term_days: number
  status: string
  idempotency_key: string
  matures_at: number
}

export interface BackendEarnSubscriptionsResponse {
  subscriptions: BackendEarnSubscription[]
}

export interface PcFinanceProduct {
  id: number
  status: number
  step: number
  acceptUnit: string
  maxLimitAmount: number
  minLimitAmount: number
  minDaysProfit: number
  cycle: number
  iconImageUrl: string
}

export interface PcFinanceStatistic {
  id: number
  memberId: number
  coinSymbol: string
  num: number
  earnNum: number
}

export interface PcFinanceOrder {
  id: number
  memberId: number
  financeId: number
  coinSymbol: string
  num: number
  status: number
  earnNum: number
  hourCursor: number
  cycle: number
  breachFee: number
  minDaysProfit: number
  maxDaysProfit: number
  createTime: number
  updateTime: number
}

export interface BackendNewCoinProject {
  id: number
  asset_id: number
  symbol: string
  lifecycle_status: string
  total_supply: string | number
  issue_price: string | number
  listed_at?: number | null
  unlock_type: string
  fixed_unlock_at?: number | null
  relative_unlock_seconds?: number | null
  unlock_fee_enabled: boolean
  unlock_fee_rate?: string | number | null
  unlock_fee_basis?: string | null
  unlock_fee_asset?: number | null
  status: string
}

export interface BackendNewCoinProjectsResponse {
  projects: BackendNewCoinProject[]
}

export interface PcIEOProject {
  id: number
  title: string
  titleEN?: string
  detail: string
  detailEN?: string
  smallImageUrl: string
  bannerImageUrl: string
  status: number
  step: number
  progress: number
  startTime: string
  endTime: string
  type: number
  totalSupply: number
  tradedAmount: number
  price: number
  priceScale: number
  unit: string
  acceptUnit: string
  acceptAssetId: number
  amountScale: number
  maxLimitAmout: number
  minLimitAmout: number
  holdLimit: number
  holdUnit: string
  limitTimes: number
  miningPeriod: number
  miningDays: number
  miningUnit: string
  lockedPeriod: number
  lockedDays: number
  releaseType: number
  releasePercent: number
  releaseAmount: number
  content: string
  contentEN?: string
}

export interface BackendCreateNewCoinSubscriptionRequest {
  quote_asset_id: number
  quote_amount: string
  quantity: string
  idempotency_key: string
}

export interface BackendSecondsProduct {
  id: number
  pair_id: number
  symbol: string
  stake_asset: number
  stake_asset_symbol: string
  duration_seconds: number
  payout_rate: string | number
  min_stake: string | number
  max_stake?: string | number | null
  status: string
}

export interface BackendSecondsProductsResponse {
  products: BackendSecondsProduct[]
}

export interface BackendCreateSecondsOrderRequest {
  product_id: number
  direction: string
  stake_amount: string
  idempotency_key: string
}

export interface BackendSecondsOrder {
  id: number
  user_id: number
  product_id: number
  pair_id: number
  symbol?: string
  stake_asset: number
  stake_asset_symbol?: string
  direction: string
  stake_amount: string | number
  payout_rate: string | number
  entry_price?: string | number | null
  status: string
  result?: string | null
  idempotency_key: string
  expires_at: number
}

export interface BackendSecondsOrdersResponse {
  orders: BackendSecondsOrder[]
}

export interface BackendMarginProduct {
  id: number
  pair_id: number
  symbol: string
  margin_asset: number
  margin_asset_symbol: string
  margin_mode: string
  leverage_levels: string[] | string
  max_leverage: string | number
  min_margin: string | number
  max_margin?: string | number | null
  maintenance_margin_rate: string | number
  hourly_interest_rate: string | number
  status: string
}

export interface BackendMarginProductsResponse {
  products: BackendMarginProduct[]
}

export interface BackendCreateMarginPositionRequest {
  product_id: number
  direction: string
  margin_amount: string
  leverage: string
  idempotency_key: string
}

export interface BackendMarginPosition {
  id: number
  user_id: number
  product_id: number
  pair_id: number
  symbol?: string
  margin_asset: number
  margin_asset_symbol?: string
  margin_mode: string
  direction: string
  margin_amount: string | number
  leverage: string | number
  notional_amount: string | number
  borrowed_amount: string | number
  interest_amount: string | number
  entry_price?: string | number | null
  exit_price?: string | number | null
  realized_pnl?: string | number | null
  closed_at?: number | null
  status: string
  idempotency_key: string
}

export interface BackendMarginPositionsResponse {
  positions: BackendMarginPosition[]
}

export interface BackendMyInviteUser {
  user_id: number
  email?: string | null
  phone?: string | null
  status: string
  direct_inviter_type?: string | null
  direct_inviter_id?: number | null
  root_agent_id?: number | null
  depth: number
  path: string
  created_at: number
}

export interface BackendMyInvitesResponse {
  users: BackendMyInviteUser[]
}

export interface PcInviteRecord {
  date: string
  invitee: string
  status: string
  reward: string
}

export interface BackendNewsContentTranslation {
  locale: string
  summary?: string | null
  content?: string | null
}

export interface BackendNewsContentDocument {
  version?: number
  default_locale?: string
  items?: BackendNewsContentTranslation[]
}

export interface BackendPublicNewsItem {
  id: number
  title: string
  category: string
  status: string
  country_code?: string | null
  default_locale?: string
  content_json?: BackendNewsContentDocument | null
  published_at?: number | null
  created_at: number
  updated_at: number
}

export interface BackendPublicNewsItemsResponse {
  news: BackendPublicNewsItem[]
}

export interface PcNewsCard {
  id: number
  title: string
  summary: string
  category: string
  time: string
  source: string
}

export function backendApiUrl(path: string): string {
  const domain = APP_CONFIG.BACKEND_API_DOMAIN.replace(/\/$/, '')
  const prefix = APP_CONFIG.BACKEND_API_PREFIX.replace(/\/$/, '')
  const suffix = path.startsWith('/') ? path : `/${path}`
  return `${domain}${prefix}${suffix}`
}

export function createAuthorizationHeader(token: string): string {
  return `Bearer ${token}`
}

export function normalizeAuthResponse(response: BackendAuthTokenResponse): PcAuthResponse {
  return {
    code: 0,
    message: 'success',
    data: {
      token: response.access_token,
      accessToken: response.access_token,
      refreshToken: response.refresh_token,
      tokenType: response.token_type,
      scope: response.scope,
    },
  }
}

export function normalizeProfileForSecurity(profile: BackendUserProfile): PcApiResponse<PcSecurityProfile> {
  return {
    code: 0,
    message: 'success',
    data: {
      id: profile.id,
      username: profile.email || profile.phone || String(profile.id),
      createTime: formatUnixMillis(profile.created_at),
      email: profile.email || undefined,
      phone: profile.phone || undefined,
      emailVerified: profile.email_verified_at ? 1 : 0,
      phoneVerified: profile.phone ? 1 : 0,
      fundsVerified: profile.fund_password_set ? 1 : 0,
      transactionStatus: profile.fund_password_set ? 1 : 0,
      realVerified: profile.kyc_level > 0 ? 1 : 0,
      realAuditing: 0,
    },
  }
}

export function mapWalletAccountsToMemberWallets(
  response: BackendWalletAccountsResponse,
): PcApiResponse<PcMemberWallet[]> {
  return {
    code: 0,
    message: 'success',
    data: response.accounts.map((account) => {
      const symbol = account.symbol.toUpperCase()
      return {
        id: account.asset_id,
        memberId: account.user_id,
        coin: {
          name: symbol,
          unit: symbol,
          coinGroup: symbol,
          canWithdraw: true,
          canRecharge: true,
        },
        balance: toNumber(account.available),
        frozenBalance: toNumber(account.frozen) + toNumber(account.locked),
        address: '',
      }
    }),
  }
}

export function mapMarketsToPcTickers(
  response: BackendMarketListResponse,
  tickersBySymbol: Record<string, BackendMarketTicker> = {},
): PcMarketTicker[] {
  return response.markets.map((market) => {
    const symbol = displayMarketSymbol(market)
    const compactSymbol = compactMarketSymbol(symbol)
    const ticker = tickersBySymbol[compactSymbol] || tickersBySymbol[market.symbol] || tickersBySymbol[market.symbol.replace(/[-_/]/g, '')]
    const close = toNumber(ticker?.last_price ?? 0)
    const volume = toNumber(ticker?.volume_24h ?? 0)

    return {
      symbol,
      icon: '',
      open: close,
      high: close,
      low: close,
      close,
      volume,
      turnover: close * volume,
      time: ticker?.observed_at ?? 0,
      chg: 0,
      zone: market.market_type === 'strategy' ? 1 : 0,
    }
  })
}

export function mapMarketTickerToPcTicker(current: PcMarketTicker | undefined, ticker: BackendMarketTicker): PcMarketTicker {
  const close = toNumber(ticker.last_price)
  const volume = toNumber(ticker.volume_24h)
  const open = current?.open || close
  return {
    symbol: current?.symbol || displaySymbolFromCompact(ticker.symbol),
    icon: current?.icon || '',
    open,
    high: Math.max(current?.high || close, close),
    low: current?.low ? Math.min(current.low, close) : close,
    close,
    volume,
    turnover: close * volume,
    time: ticker.observed_at,
    chg: open ? ((close - open) / open) * 100 : 0,
    zone: current?.zone || 0,
  }
}

export function mapMarketKlinesToPcRows(rows: BackendMarketKline[]): number[][] {
  return rows.map((row) => [
    row.open_time,
    toNumber(row.open),
    toNumber(row.high),
    toNumber(row.low),
    toNumber(row.close),
    toNumber(row.volume),
  ])
}

export function mapMarketDepthToTradePlate(depth: BackendMarketDepth): PcMarketDepth {
  return {
    symbol: displaySymbolFromCompact(depth.symbol),
    bids: depth.bids.map(mapMarketDepthLevel),
    asks: depth.asks.map(mapMarketDepthLevel),
    time: depth.observed_at,
  }
}

export function mapMarketTradeToPcTrade(trade: BackendMarketTrade): PcMarketTrade {
  const direction = String(trade.direction || trade.side || 'BUY').toUpperCase() === 'SELL' ? 'SELL' : 'BUY'
  return {
    id: trade.id,
    symbol: trade.symbol,
    direction,
    price: toNumber(trade.price),
    amount: toNumber(trade.amount ?? trade.quantity ?? 0),
    time: Number(trade.time ?? trade.traded_at ?? 0),
  }
}

export function mapSpotOrdersToPcPage(
  response: BackendSpotOrdersResponse,
  params: { pageNo: number; pageSize: number },
): PcApiResponse<PcPageData<PcSpotOrderRow>> {
  const content = response.orders.map(mapSpotOrderToPcRow)
  return {
    code: 0,
    message: 'success',
    data: {
      content,
      page: {
        number: Math.max(params.pageNo, 0),
        size: params.pageSize,
        totalElements: content.length,
        totalPages: Math.max(Math.ceil(content.length / params.pageSize), 1),
      },
    },
  }
}

export function mapPcSpotOrderRequest(params: PcSpotOrderParams, idempotencyKey: string): BackendCreateSpotOrderRequest {
  const orderType = params.type === 'MARKET_PRICE' ? 'market' : 'limit'
  const side = params.direction === 'SELL' ? 'sell' : 'buy'
  const referencePrice = params.price ?? 0
  const request: BackendCreateSpotOrderRequest = {
    pair_id: marketPairId(params.symbol),
    side,
    order_type: orderType,
    quantity: String(orderType === 'market' && side === 'buy' && referencePrice > 0 ? params.amount / referencePrice : params.amount),
    idempotency_key: idempotencyKey,
  }

  if (orderType === 'limit') {
    request.price = String(params.price ?? 0)
  } else {
    request.reference_price = String(referencePrice)
  }

  return request
}

export function mapWalletAccountsToTradeWallets(
  response: BackendWalletAccountsResponse,
  symbol: string,
): PcTradeWalletBalance[] {
  const wanted = marketPairId(symbol).split('-')
  return response.accounts
    .filter((account) => wanted.includes(account.symbol.toUpperCase()))
    .map((account) => ({
      symbol: account.symbol.toUpperCase(),
      balance: toNumber(account.available),
      frozenBalance: toNumber(account.frozen) + toNumber(account.locked),
    }))
}

export function normalizeWalletLedgerPage(
  response: BackendWalletLedgerResponse,
  params: { pageNo: number; pageSize: number },
): PcApiResponse<PcPageData<PcTransactionRecord>> {
  const content = response.entries.map((entry) => ({
    id: entry.id,
    memberId: entry.user_id,
    amount: signedAmount(entry),
    fee: 0,
    symbol: entry.symbol,
    type: transactionTypeForRef(entry.ref_type),
    createTime: formatUnixMillis(entry.created_at),
    status: 1,
  }))

  return {
    code: 0,
    message: 'success',
    data: {
      content,
      page: {
        number: Math.max(params.pageNo - 1, 0),
        size: params.pageSize,
        totalElements: content.length,
        totalPages: Math.max(Math.ceil(content.length / params.pageSize), 1),
      },
    },
  }
}

export function mapConvertPairsToPcCoins(response: BackendConvertPairsResponse): PcApiResponse<PcSwapCoinRow[]> {
  return {
    code: 0,
    message: 'success',
    data: response.pairs.map((pair) => ({
      id: pair.id,
      fromUnit: pair.from_asset_symbol || String(pair.from_asset_id),
      toUnit: pair.to_asset_symbol || String(pair.to_asset_id),
      minAmount: toNumber(pair.min_amount),
      maxAmount: toNumber(pair.max_amount ?? 0),
      enabled: pair.enabled,
    })),
  }
}

export function mapEarnProductsToPcFinanceList(response: BackendEarnProductsResponse): PcApiResponse<PcFinanceProduct[]> {
  return {
    code: 0,
    message: 'success',
    data: response.products.map((product) => ({
      id: product.id,
      status: product.status === 'active' ? 1 : 0,
      step: product.status === 'active' ? 1 : 3,
      acceptUnit: product.asset_symbol,
      maxLimitAmount: toNumber(product.max_subscribe ?? 0),
      minLimitAmount: toNumber(product.min_subscribe),
      minDaysProfit: toNumber(product.apr_rate),
      cycle: product.term_days,
      iconImageUrl: '',
    })),
  }
}

export function mapEarnSubscriptionsToPcFinanceStatistic(
  response: BackendEarnSubscriptionsResponse,
  symbol = 'USDT',
): PcApiResponse<PcFinanceStatistic> {
  const active = response.subscriptions.filter((item) => financeSubscriptionStatusToPc(item.status) === 0)
  return {
    code: 0,
    message: 'success',
    data: {
      id: 0,
      memberId: active[0]?.user_id ?? 0,
      coinSymbol: symbol,
      num: active.length,
      earnNum: response.subscriptions.reduce((sum, item) => sum + projectedEarnAmount(item), 0),
    },
  }
}

export function mapEarnSubscriptionsToPcFinanceCount(response: BackendEarnSubscriptionsResponse): PcApiResponse<number> {
  return {
    code: 0,
    message: 'success',
    data: response.subscriptions
      .filter((item) => financeSubscriptionStatusToPc(item.status) === 0)
      .reduce((sum, item) => sum + toNumber(item.amount), 0),
  }
}

export function mapEarnSubscriptionsToPcFinancePage(
  response: BackendEarnSubscriptionsResponse,
  params: { pageNo: number; pageSize: number; status?: number },
): PcApiResponse<PcPageData<PcFinanceOrder>> {
  const filtered = response.subscriptions
    .filter((item) => params.status === undefined || financeSubscriptionStatusToPc(item.status) === params.status)
    .map(mapEarnSubscriptionToPcOrder)
  return {
    code: 0,
    message: 'success',
    data: {
      content: filtered,
      page: {
        number: Math.max(params.pageNo - 1, 0),
        size: params.pageSize,
        totalElements: filtered.length,
        totalPages: Math.max(Math.ceil(filtered.length / params.pageSize), 1),
      },
    },
  }
}

export function mapNewCoinProjectsToPcActivityPage(
  response: BackendNewCoinProjectsResponse,
  params: { pageNo: number; pageSize: number; step?: number },
): PcApiResponse<PcPageData<PcIEOProject>> {
  const content = response.projects
    .map(mapNewCoinProjectToPcActivity)
    .filter((project) => params.step === undefined || params.step < 0 || project.step === params.step)
  return {
    code: 0,
    message: 'success',
    data: {
      content,
      page: {
        number: Math.max(params.pageNo - 1, 0),
        size: params.pageSize,
        totalElements: content.length,
        totalPages: Math.max(Math.ceil(content.length / params.pageSize), 1),
      },
    },
  }
}

export function mapPcNewCoinSubscriptionRequest(
  params: { quoteAssetId: number; amount: number; price: number },
  idempotencyKey: string,
): BackendCreateNewCoinSubscriptionRequest {
  return {
    quote_asset_id: params.quoteAssetId,
    quote_amount: String(params.amount),
    quantity: String(params.price > 0 ? params.amount / params.price : 0),
    idempotency_key: idempotencyKey,
  }
}

export function mapSecondsProductsToPcCycles(response: BackendSecondsProductsResponse): PcApiResponse<Array<{ id: number; cycleLength: number; cycleRate: number; minAmount: number; maxAmount: number }>> {
  return {
    code: 0,
    message: 'success',
    data: response.products.map((product) => ({
      id: product.id,
      cycleLength: product.duration_seconds,
      cycleRate: toNumber(product.payout_rate),
      minAmount: toNumber(product.min_stake),
      maxAmount: toNumber(product.max_stake ?? 0),
    })),
  }
}

export function mapPcSecondsOrderRequest(
  params: { cycleId: number; direction: 0 | 1; amount: number },
  idempotencyKey: string,
): BackendCreateSecondsOrderRequest {
  return {
    product_id: params.cycleId,
    direction: params.direction === 1 ? 'down' : 'up',
    stake_amount: String(params.amount),
    idempotency_key: idempotencyKey,
  }
}

export function mapSecondsOrdersToPcOrders(response: BackendSecondsOrdersResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: response.orders.map((order) => ({
      id: order.id,
      symbol: order.symbol || String(order.pair_id),
      coinSymbol: order.stake_asset_symbol || String(order.stake_asset),
      direction: order.direction === 'down' ? 'SELL' : 'BUY',
      amount: toNumber(order.stake_amount),
      betAmount: toNumber(order.stake_amount),
      openPrice: toNumber(order.entry_price ?? 0),
      closePrice: 0,
      cycleLength: 0,
      cycleRate: toNumber(order.payout_rate),
      status: secondsStatusToPc(order.status),
      result: secondsResultToPc(order.result),
      profit: secondsProfit(order),
      createTime: 0,
      endTime: order.expires_at,
    })),
  }
}

export function mapMarginProductsToContractCoins(response: BackendMarginProductsResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: response.products.map((product) => ({
      id: product.id,
      symbol: displaySymbolFromCompact(product.symbol),
      baseSymbol: product.margin_asset_symbol,
      coinSymbol: displaySymbolFromCompact(product.symbol).split('/')[0],
      enable: product.status === 'active',
      sort: product.id,
      minTurnover: toNumber(product.min_margin),
      maxLeverage: toNumber(product.max_leverage),
      marginRate: toNumber(product.maintenance_margin_rate),
      leverage: parseLeverageLevels(product.leverage_levels),
    })),
  }
}

export function mapPcMarginOpenRequest(
  params: { contractCoinId: number; direction: 0 | 1; leverage: number; volume: number },
  idempotencyKey: string,
): BackendCreateMarginPositionRequest {
  return {
    product_id: params.contractCoinId,
    direction: params.direction === 1 ? 'short' : 'long',
    margin_amount: String(params.volume),
    leverage: String(params.leverage),
    idempotency_key: idempotencyKey,
  }
}

export function mapMarginPositionsToContractOrders(response: BackendMarginPositionsResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: response.positions.map((position) => ({
      orderId: String(position.id),
      productId: position.product_id,
      symbol: displaySymbolFromCompact(position.symbol || String(position.pair_id)),
      price: toNumber(position.entry_price ?? position.exit_price ?? 0),
      amount: toNumber(position.notional_amount),
      direction: position.direction === 'short' ? 1 : 0,
      type: 0,
      leverage: toNumber(position.leverage),
      tradedAmount: toNumber(position.notional_amount),
      status: marginPositionStatusToPc(position.status),
      createTime: 0,
    })),
  }
}

export function mapMarginPositionsToContractWallets(response: BackendMarginPositionsResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: response.positions.map(mapMarginPositionToContractWallet),
  }
}

export function mapMyInvitesToPcInviteRecords(response: BackendMyInvitesResponse): PcApiResponse<PcInviteRecord[]> {
  return {
    code: 0,
    message: 'success',
    data: response.users.map((user) => ({
      date: formatUnixMillis(user.created_at),
      invitee: maskInvitee(user.email || user.phone || String(user.user_id)),
      status: user.status === 'active' ? 'Completed' : 'Registered',
      reward: '-',
    })),
  }
}

export function mapPublicNewsItemsToPcNewsCards(response: BackendPublicNewsItemsResponse): PcNewsCard[] {
  return response.news.map((item) => {
    const content = selectNewsContent(item)
    return {
      id: item.id,
      title: item.title,
      summary: content.summary || content.content || '',
      category: newsCategoryToPc(item.category),
      time: formatUnixMillis(item.published_at || item.updated_at || item.created_at),
      source: 'MarketEx Team',
    }
  })
}

function projectedEarnAmount(item: BackendEarnSubscription): number {
  return toNumber(item.amount) * toNumber(item.apr_rate) * (item.term_days / 365)
}

function mapEarnSubscriptionToPcOrder(item: BackendEarnSubscription): PcFinanceOrder {
  return {
    id: item.id,
    memberId: item.user_id,
    financeId: item.product_id,
    coinSymbol: item.asset_symbol || String(item.asset_id),
    num: toNumber(item.amount),
    status: financeSubscriptionStatusToPc(item.status),
    earnNum: projectedEarnAmount(item),
    hourCursor: 0,
    cycle: item.term_days,
    breachFee: 0,
    minDaysProfit: toNumber(item.apr_rate),
    maxDaysProfit: toNumber(item.apr_rate),
    createTime: 0,
    updateTime: item.matures_at,
  }
}

function financeSubscriptionStatusToPc(status: string): number {
  const normalized = status.toLowerCase()
  return normalized === 'active' || normalized === 'subscribed' ? 0 : 1
}

function mapNewCoinProjectToPcActivity(project: BackendNewCoinProject): PcIEOProject {
  const step = newCoinStatusToPcStep(project.lifecycle_status || project.status)
  const endTime = formatUnixMillis(project.fixed_unlock_at || project.listed_at || 0)
  return {
    id: project.id,
    title: project.symbol,
    detail: project.lifecycle_status,
    smallImageUrl: '',
    bannerImageUrl: '',
    status: project.status === 'active' ? 1 : 0,
    step,
    progress: 0,
    startTime: formatUnixMillis(project.listed_at || 0),
    endTime,
    type: 4,
    totalSupply: toNumber(project.total_supply),
    tradedAmount: 0,
    price: toNumber(project.issue_price),
    priceScale: 8,
    unit: project.symbol,
    acceptUnit: 'USDT',
    acceptAssetId: project.asset_id,
    amountScale: 8,
    maxLimitAmout: 0,
    minLimitAmout: 0,
    holdLimit: 0,
    holdUnit: '',
    limitTimes: 0,
    miningPeriod: 0,
    miningDays: 0,
    miningUnit: '',
    lockedPeriod: 0,
    lockedDays: 0,
    releaseType: project.unlock_type === 'fixed' ? 1 : 2,
    releasePercent: 0,
    releaseAmount: 0,
    content: project.lifecycle_status,
  }
}

function newCoinStatusToPcStep(status: string): number {
  const normalized = status.toLowerCase()
  if (normalized === 'preheat' || normalized.includes('draft') || normalized.includes('pending') || normalized.includes('upcoming')) return 0
  if (normalized === 'subscription' || normalized.includes('open') || normalized.includes('sale')) return 1
  if (normalized === 'distribution' || normalized.includes('distribut')) return 2
  return 3
}

function secondsStatusToPc(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'open' || normalized === 'opened' || normalized === 'pending') return 'OPEN'
  if (normalized === 'cancelled' || normalized === 'canceled') return 'CANCELED'
  return 'CLOSE'
}

function secondsResultToPc(result?: string | null): string | number {
  if (!result) return 0
  const normalized = result.toLowerCase()
  if (normalized === 'win') return 'WIN'
  if (normalized === 'loss' || normalized === 'lose') return 'LOSE'
  return result.toUpperCase()
}

function secondsProfit(order: BackendSecondsOrder): number {
  if (order.result?.toLowerCase() === 'win') return toNumber(order.stake_amount) * toNumber(order.payout_rate)
  if (order.result?.toLowerCase() === 'loss' || order.result?.toLowerCase() === 'lose') return -toNumber(order.stake_amount)
  return 0
}

function marginPositionStatusToPc(status: string): number {
  const normalized = status.toLowerCase()
  return normalized === 'open' || normalized === 'opened' ? 0 : 1
}

function parseLeverageLevels(levels: string[] | string): number[] {
  const values = Array.isArray(levels) ? levels : levels.split(',')
  return values.map((level) => toNumber(level)).filter((level) => level > 0)
}

function mapMarginPositionToContractWallet(position: BackendMarginPosition) {
  const symbol = displaySymbolFromCompact(position.symbol || String(position.pair_id))
  const isShort = position.direction === 'short'
  return {
    id: position.id,
    memberId: position.user_id,
    symbol,
    coinSymbol: symbol.split('/')[0],
    baseSymbol: position.margin_asset_symbol || String(position.margin_asset),
    usdtBalance: toNumber(position.margin_amount),
    usdtFrozenBalance: 0,
    usdtBuyPosition: isShort ? 0 : toNumber(position.notional_amount),
    usdtBuyPrice: isShort ? 0 : toNumber(position.entry_price ?? 0),
    usdtBuyLeverage: isShort ? 1 : toNumber(position.leverage),
    usdtBuyPrincipalAmount: isShort ? 0 : toNumber(position.margin_amount),
    usdtFrozenBuyPosition: 0,
    usdtSellPosition: isShort ? toNumber(position.notional_amount) : 0,
    usdtSellPrice: isShort ? toNumber(position.entry_price ?? 0) : 0,
    usdtSellLeverage: isShort ? toNumber(position.leverage) : 1,
    usdtSellPrincipalAmount: isShort ? toNumber(position.margin_amount) : 0,
    usdtFrozenSellPosition: 0,
    usdtShareNumber: 1,
    usdtPattern: position.margin_mode === 'cross' ? 'CROSSED' : 'FIXED',
    usdtTotalProfitAndLoss: toNumber(position.realized_pnl ?? 0),
    currentPrice: toNumber(position.entry_price ?? 0),
    closeFee: 0,
    maintenanceMarginRate: 0,
  }
}

function displayMarketSymbol(market: BackendMarket): string {
  if (market.base_asset && market.quote_asset) return `${market.base_asset}/${market.quote_asset}`
  return displaySymbolFromCompact(market.symbol)
}

function mapSpotOrderToPcRow(order: BackendSpotOrder): PcSpotOrderRow {
  return {
    orderId: order.id,
    symbol: displaySymbolFromCompact(order.pair_id),
    direction: order.side === 'sell' ? 'SELL' : 'BUY',
    type: order.order_type === 'market' ? 'MARKET_PRICE' : 'LIMIT_PRICE',
    price: toNumber(order.price ?? 0),
    amount: toNumber(order.quantity),
    filledAmount: toNumber(order.filled_quantity),
    status: spotOrderStatusToPc(order.status),
    time: 0,
  }
}

function spotOrderStatusToPc(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'open' || normalized === 'pending' || normalized === 'partially_filled') return 'TRADING'
  if (normalized === 'cancelled') return 'CANCELED'
  if (normalized === 'filled') return 'COMPLETED'
  if (normalized === 'rejected') return 'REJECTED'
  return normalized.toUpperCase()
}

function marketPairId(symbol: string): string {
  return symbol.replace('/', '-').replace('_', '-').toUpperCase()
}

function displaySymbolFromCompact(symbol: string): string {
  const normalized = symbol.toUpperCase()
  if (normalized.includes('/')) return normalized
  if (normalized.includes('-')) return normalized.replace('-', '/')
  if (normalized.endsWith('USDT') && normalized.length > 4) {
    return `${normalized.slice(0, -4)}/USDT`
  }
  return normalized.replace('_', '/')
}

function compactMarketSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function mapMarketDepthLevel(level: BackendMarketDepthLevel): PcMarketDepthLevel {
  return {
    price: toNumber(level.price),
    amount: toNumber(level.amount ?? level.quantity ?? 0),
  }
}

function signedAmount(entry: BackendWalletLedgerEntry): number {
  const amount = toNumber(entry.amount)
  return entry.change_type === 'debit' ? -Math.abs(amount) : amount
}

function transactionTypeForRef(refType: string): number {
  if (refType.includes('withdraw')) return 1
  if (refType.includes('convert') || refType.includes('swap')) return 3
  if (refType.includes('admin_recharge') || refType.includes('recharge')) return 10
  return 2
}

function maskInvitee(value: string): string {
  if (value.includes('@')) {
    const [name, domain] = value.split('@')
    return `${name.slice(0, 1)}***@${domain}`
  }
  if (value.length <= 4) return value
  return `${value.slice(0, 3)}****${value.slice(-2)}`
}

function selectNewsContent(item: BackendPublicNewsItem): BackendNewsContentTranslation {
  const items = item.content_json?.items ?? []
  const locale = item.default_locale || item.content_json?.default_locale
  return items.find((entry) => entry.locale === locale) || items[0] || { locale: locale || 'en' }
}

function newsCategoryToPc(category: string): string {
  const normalized = category.toLowerCase()
  if (normalized === 'system' || normalized === 'promotion' || normalized === 'product') return 'announcement'
  if (normalized === 'market') return 'flash'
  return 'deep'
}

function toNumber(value: string | number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function formatUnixMillis(value: number): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (part: number) => String(part).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}
