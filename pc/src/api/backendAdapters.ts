import { APP_CONFIG } from '../config/app.ts'
import { formatBusinessOrderNo } from '../utils/orderNo.ts'

export interface BackendAuthTokenResponse {
  access_token: string
  refresh_token: string
  token_type: string
  scope: string
}

export interface BackendLoginTwoFactorChallengeResponse {
  requires_2fa: true
  challenge_id: string
  expires_in_seconds: number
}

export interface BackendLoginTwoFactorSetupChallengeResponse {
  requires_2fa_setup: true
  setup_challenge_id: string
  expires_in_seconds: number
}

export type BackendAuthResponse =
  | BackendAuthTokenResponse
  | BackendLoginTwoFactorChallengeResponse
  | BackendLoginTwoFactorSetupChallengeResponse

export interface PcAuthResponse {
  code: number
  message: string
  data: {
    token?: string
    accessToken?: string
    refreshToken?: string
    tokenType?: string
    scope?: string
    requires2fa?: boolean
    challengeId?: string
    requires2faSetup?: boolean
    setupChallengeId?: string
    expiresInSeconds?: number
  }
}

export type PcLocale = 'en' | 'zh'

export interface ProfileLocaleSource {
  preferredLocale?: string | null
  defaultLocale?: string | null
  supportedLocales?: string[] | null
}

export interface BackendUserProfile {
  id: number
  username?: string | null
  email?: string | null
  phone?: string | null
  avatar_url?: string | null
  country_code?: string | null
  preferred_locale?: string | null
  default_locale?: string | null
  supported_locales?: string[] | null
  status: string
  kyc_level: number
  email_verified_at?: number | null
  fund_password_set: boolean
  created_at: number
}

export interface PcSecurityProfile extends ProfileLocaleSource {
  username?: string
  id: number
  createTime: string
  avatar?: string
  email?: string
  phone?: string
  realName?: string
  idCard?: string
  kycLevel: number
  countryCode?: string
  preferredLocale?: PcLocale
  defaultLocale?: PcLocale
  supportedLocales: PcLocale[]
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
  logo_url?: string | null
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
  logoUrl?: string
}

export interface PcMemberWallet {
  id: string | number
  memberId: string | number
  coin: PcCoin
  balance: number
  frozenBalance: number
  address: string
}

export interface BackendCreateWithdrawalRequest {
  asset_symbol: string
  network?: string
  address: string
  amount: string
  fee: string
  fund_password?: string
  totp_code?: string
}

export interface PcWithdrawalParams {
  unit: string
  network?: string
  address: string
  amount: number
  fee: number
  code?: string
  fundPassword?: string
  totpCode?: string
}

export interface BackendWalletLedgerResponse {
  entries: BackendWalletLedgerEntry[]
  page?: BackendWalletLedgerPage
}

export interface BackendWalletLedgerPage {
  number?: number
  size?: number
  total_elements?: number
  totalElements?: number
  total_pages?: number
  totalPages?: number
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
  fee?: string | number | null
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
  type: string
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
  logo_url?: string | null
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
  high_24h?: string | number | null
  low_24h?: string | number | null
  volume_24h: string | number
  price_change_24h?: string | number | null
  price_change_percent_24h?: string | number | null
  open_24h?: string | number | null
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
  order_type: 'limit' | 'market' | 'stop_limit'
  price?: string | number | null
  trigger_price?: string | number | null
  quantity: string | number
  filled_quantity: string | number
  average_price?: string | number | null
  status: string
  created_at?: number
}

export interface BackendSpotOrdersResponse {
  orders: BackendSpotOrder[]
}

export interface PcSpotOrderRow {
  orderId: string
  symbol: string
  direction: 'BUY' | 'SELL'
  type: 'LIMIT_PRICE' | 'MARKET_PRICE' | 'STOP_LIMIT'
  price: number
  triggerPrice?: number | null
  amount: number
  filledAmount: number
  filledPrice: number | null
  status: string
  time: number
}

export interface PcSpotOrderParams {
  symbol: string
  price?: number
  triggerPrice?: number
  amount: number
  direction: 'BUY' | 'SELL'
  type: 'LIMIT_PRICE' | 'MARKET_PRICE' | 'STOP_LIMIT'
}

export interface BackendCreateSpotOrderRequest {
  pair_id: string
  side: 'buy' | 'sell'
  order_type: 'limit' | 'market' | 'stop_limit'
  price?: string
  trigger_price?: string
  quantity: string
  reference_price?: string
  idempotency_key: string
}

export interface PcTradeWalletBalance {
  symbol: string
  balance: number
  frozenBalance: number
  logoUrl?: string
}

export interface BackendConvertPair {
  id: number
  from_asset_id: number
  from_asset_symbol?: string
  to_asset_id: number
  to_asset_symbol?: string
  pricing_mode: string
  spread_rate: string | number
  fee_rate: string | number
  min_amount: string | number
  max_amount?: string | number | null
  target_min_amount?: string | number | null
  target_max_amount?: string | number | null
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
  feeRate: number
  enabled: boolean
}

export interface PcSwapPairOption extends PcSwapCoinRow {
  fromAssetId: number
  toAssetId: number
}

export interface BackendConvertQuote {
  quote_id: string
  convert_pair_id: number
  from_asset_id: number
  to_asset_id: number
  from_amount: string | number
  to_amount: string | number
  rate: string | number
  spread_rate: string | number
  fee_rate: string | number
  fee_amount: string | number
  expires_at: number
}

export interface PcSwapQuote {
  quoteId: string
  pairId: number
  fromAssetId: number
  toAssetId: number
  fromAmount: number
  toAmount: number
  rate: number
  spreadRate: number
  feeRate: number
  feeAmount: number
  expiresAt: number
}

export interface BackendConvertOrder {
  id: number
  quote_id: string
  convert_pair_id: number
  from_asset_id: number
  to_asset_id: number
  from_amount: string | number
  to_amount: string | number
  rate: string | number
  fee_rate?: string | number | null
  fee_amount?: string | number | null
  status: string
  created_at: number
}

export interface BackendConvertOrdersResponse {
  orders: BackendConvertOrder[]
}

export interface PcSwapOrderRow {
  id: number
  quoteId: string
  pairId: number
  fromAssetId: number
  toAssetId: number
  fromUnit: string
  toUnit: string
  fromAmount: number
  toAmount: number
  rate: number
  feeRate: number
  feeAmount: number
  status: string
  createdAt: number
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
  redemption_fee_rate?: string | number
  maturity_profit_fee_rate?: string | number
  early_redeem_fee_basis?: string
  early_redeem_fee_rate?: string | number
  min_subscribe: string | number
  max_subscribe?: string | number | null
  status: string
}

export interface BackendEarnProductsResponse {
  products: BackendEarnProduct[]
}

export interface BackendEarnSubscription {
  id: number
  order_no?: string | null
  user_id: number
  product_id: number
  asset_id: number
  asset_symbol?: string
  amount: string | number
  apr_rate: string | number
  redemption_fee_rate?: string | number
  maturity_profit_fee_rate?: string | number
  early_redeem_fee_basis?: string
  early_redeem_fee_rate?: string | number
  term_days: number
  status: string
  idempotency_key: string
  subscribed_at?: number
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
  orderNo: string
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
  logo_url?: string | null
  stake_asset: number
  stake_asset_symbol: string
  duration_seconds: number
  payout_rate: string | number
  min_stake: string | number
  max_stake?: string | number | null
  cycles?: BackendSecondsProductCycle[]
  status: string
}

export interface BackendSecondsProductCycle {
  id: number
  product_id: number
  duration_seconds: number
  payout_rate: string | number
  min_stake: string | number
  max_stake?: string | number | null
  sort_order?: number
}

export interface BackendSecondsProductsResponse {
  products: BackendSecondsProduct[]
}

export interface BackendCreateSecondsOrderRequest {
  product_id: number
  duration_seconds?: number
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
  duration_seconds?: number
  payout_rate: string | number
  entry_price?: string | number | null
  status: string
  result?: string | null
  idempotency_key: string
  expires_at: number
  created_at?: number
  opened_at?: number
  time?: number
}

export interface BackendSecondsOrdersResponse {
  orders: BackendSecondsOrder[]
}

export interface BackendMarginProduct {
  id: number
  pair_id: number
  symbol: string
  logo_url?: string | null
  margin_asset: number
  margin_asset_symbol: string
  margin_mode: string
  margin_modes?: string[] | string
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
  capabilities?: {
    margin_modes?: string[] | string
    order_types?: string[] | string
  }
}

export interface BackendCreateMarginPositionRequest {
  product_id: number
  direction: string
  order_type?: 'market'
  margin_mode?: string
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
  wallet_scope?: string
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

export interface BackendMarginWallet {
  asset_id: number
  asset_symbol: string
  available: string | number
  frozen: string | number
  locked: string | number
}

export interface BackendMarginWalletsResponse {
  wallets: BackendMarginWallet[]
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

export interface BackendPublicCountry {
  country_code: string
  country_name: string
  default_locale: string
  supported_locales: string[]
}

export interface BackendPublicCountriesResponse {
  countries: BackendPublicCountry[]
}

export interface PcCountryOption {
  code: string
  name: string
  defaultLocale: PcLocale
  supportedLocales: PcLocale[]
}

export interface BackendNewsContentTranslation {
  locale: string
  title?: string | null
  summary?: BackendNewsRichTextValue
  content?: BackendNewsRichTextValue
}

type BackendNewsRichTextValue = string | BackendNewsRichTextBlock[] | null | undefined

export interface BackendNewsRichTextLeaf {
  text: string
  bold?: boolean | null
  italic?: boolean | null
  underline?: boolean | null
}

export interface BackendNewsRichTextBlock {
  type: 'p' | 'h1' | 'h2' | 'h3' | 'blockquote' | string
  children?: BackendNewsRichTextLeaf[] | null
  url?: string | null
  alt?: string | null
}

export interface BackendNewsContentDocument {
  version?: number
  default_locale?: string
  items?: BackendNewsContentTranslation[]
}

export interface BackendPublicNewsItem {
  id: number
  title: string
  banner_url?: string | null
  small_logo_url?: string | null
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
  content: string
  category: string
  time: string
  source: string
  bannerUrl?: string
  smallLogoUrl?: string
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

export function normalizeAuthResponse(response: BackendAuthResponse): PcAuthResponse {
  if ('requires_2fa' in response) {
    return {
      code: 0,
      message: 'two_factor_required',
      data: {
        requires2fa: true,
        challengeId: response.challenge_id,
        expiresInSeconds: response.expires_in_seconds,
      },
    }
  }

  if ('requires_2fa_setup' in response) {
    return {
      code: 0,
      message: 'two_factor_setup_required',
      data: {
        requires2faSetup: true,
        setupChallengeId: response.setup_challenge_id,
        expiresInSeconds: response.expires_in_seconds,
      },
    }
  }

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
  const supportedLocales = normalizeSupportedLocales(profile.supported_locales)
  return {
    code: 0,
    message: 'success',
    data: {
      id: profile.id,
      username: profile.username?.trim() || profile.email || profile.phone || String(profile.id),
      createTime: formatUnixMillis(profile.created_at),
      avatar: typeof profile.avatar_url === 'string' && profile.avatar_url.trim() ? profile.avatar_url.trim() : undefined,
      email: profile.email || undefined,
      phone: profile.phone || undefined,
      countryCode: profile.country_code || undefined,
      preferredLocale: normalizePcLocale(profile.preferred_locale),
      defaultLocale: normalizePcLocale(profile.default_locale),
      supportedLocales,
      emailVerified: profile.email_verified_at ? 1 : 0,
      phoneVerified: profile.phone ? 1 : 0,
      fundsVerified: profile.fund_password_set ? 1 : 0,
      transactionStatus: profile.fund_password_set ? 1 : 0,
      kycLevel: profile.kyc_level,
      realVerified: profile.kyc_level > 0 ? 1 : 0,
      realAuditing: 0,
    },
  }
}

export function mapPublicCountriesToPcOptions(response: BackendPublicCountriesResponse): PcCountryOption[] {
  return response.countries.map((country) => ({
    code: country.country_code,
    name: country.country_name,
    defaultLocale: normalizePcLocale(country.default_locale) || 'en',
    supportedLocales: normalizeSupportedLocales(country.supported_locales),
  }))
}

export function resolveProfileLocale(
  profile: ProfileLocaleSource | null | undefined,
  currentLocale: string,
  localeOverridden: boolean,
): PcLocale {
  const supportedLocales = normalizeSupportedLocales(profile?.supportedLocales)
  const current = normalizePcLocale(currentLocale)
  const preferred = normalizePcLocale(profile?.preferredLocale)
  const fallback = normalizePcLocale(profile?.defaultLocale) || 'en'

  if (localeOverridden && current && (supportedLocales.length === 0 || supportedLocales.includes(current))) {
    return current
  }

  if (localeOverridden && supportedLocales.length > 0) {
    return fallbackSupportedLocale(fallback, supportedLocales)
  }

  if (preferred && (supportedLocales.length === 0 || supportedLocales.includes(preferred))) {
    return preferred
  }

  return fallbackSupportedLocale(fallback, supportedLocales)
}

export function mapWalletAccountsToMemberWallets(
  response: BackendWalletAccountsResponse,
): PcApiResponse<PcMemberWallet[]> {
  return {
    code: 0,
    message: 'success',
    data: response.accounts.map((account) => {
      const symbol = account.symbol.toUpperCase()
      const logoUrl = typeof account.logo_url === 'string' ? account.logo_url.trim() : ''
      return {
        id: account.asset_id,
        memberId: account.user_id,
        coin: {
          name: symbol,
          unit: symbol,
          coinGroup: symbol,
          canWithdraw: true,
          canRecharge: true,
          logoUrl: logoUrl || undefined,
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
    const icon = typeof market.logo_url === 'string' ? market.logo_url.trim() : ''
    const close = toNumber(ticker?.last_price ?? 0)
    const volume = toNumber(ticker?.volume_24h ?? 0)
    const open = resolveTickerOpen(undefined, ticker, close)
    const high = resolveTickerHigh(ticker, close)
    const low = resolveTickerLow(ticker, close)
    const chg = resolveTickerChangePercent(ticker, close, open)

    return {
      symbol,
      icon,
      open,
      high,
      low,
      close,
      volume,
      turnover: close * volume,
      time: ticker?.observed_at ?? 0,
      chg,
      zone: market.market_type === 'strategy' ? 1 : 0,
    }
  })
}

export function mapSecondsProductsToPcTickers(
  response: BackendSecondsProductsResponse,
  tickersBySymbol: Record<string, BackendMarketTicker> = {},
): PcMarketTicker[] {
  const seen = new Set<string>()
  const tickers: PcMarketTicker[] = []

  for (const product of response.products) {
    if (String(product.status || 'active').toLowerCase() !== 'active') continue

    const symbol = displaySymbolFromCompact(product.symbol)
    const compactSymbol = compactMarketSymbol(symbol)
    if (seen.has(compactSymbol)) continue
    seen.add(compactSymbol)

    const ticker =
      tickersBySymbol[compactSymbol] ||
      tickersBySymbol[product.symbol] ||
      tickersBySymbol[product.symbol.replace(/[-_/]/g, '')]
    const icon = typeof product.logo_url === 'string' ? product.logo_url.trim() : ''
    const close = toNumber(ticker?.last_price ?? 0)
    const volume = toNumber(ticker?.volume_24h ?? 0)
    const open = resolveTickerOpen(undefined, ticker, close)
    const high = resolveTickerHigh(ticker, close)
    const low = resolveTickerLow(ticker, close)
    const chg = resolveTickerChangePercent(ticker, close, open)

    tickers.push({
      symbol,
      icon,
      open,
      high,
      low,
      close,
      volume,
      turnover: close * volume,
      time: ticker?.observed_at ?? 0,
      chg,
      zone: 0,
    })
  }

  return tickers
}

export function mapMarketTickerToPcTicker(current: PcMarketTicker | undefined, ticker: BackendMarketTicker): PcMarketTicker {
  const close = toNumber(ticker.last_price)
  const volume = toNumber(ticker.volume_24h)
  const open = resolveTickerOpen(current, ticker, close)
  const high = resolveTickerHigh(ticker, close, current)
  const low = resolveTickerLow(ticker, close, current)
  const chg = resolveTickerChangePercent(ticker, close, open)
  return {
    symbol: current?.symbol || displaySymbolFromCompact(ticker.symbol),
    icon: current?.icon || '',
    open,
    high,
    low,
    close,
    volume,
    turnover: close * volume,
    time: ticker.observed_at,
    chg,
    zone: current?.zone || 0,
  }
}

function resolveTickerOpen(current: PcMarketTicker | undefined, ticker: BackendMarketTicker | undefined, close: number): number {
  const open24h = firstNumber(ticker?.open_24h)
  if (open24h !== undefined) return open24h

  const priceChange = firstNumber(ticker?.price_change_24h)
  if (priceChange !== undefined) return close - priceChange

  return current?.open || close
}

function resolveTickerHigh(ticker: BackendMarketTicker | undefined, close: number, current?: PcMarketTicker): number {
  return firstNumber(ticker?.high_24h) ?? Math.max(current?.high || close, close)
}

function resolveTickerLow(ticker: BackendMarketTicker | undefined, close: number, current?: PcMarketTicker): number {
  return firstNumber(ticker?.low_24h) ?? (current?.low ? Math.min(current.low, close) : close)
}

function resolveTickerChangePercent(ticker: BackendMarketTicker | undefined, close: number, open: number): number {
  const backendChange = firstNumber(ticker?.price_change_percent_24h)
  if (backendChange !== undefined) return backendChange
  return open ? ((close - open) / open) * 100 : 0
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
  const content = response.orders
    .map(mapSpotOrderToPcRow)
    .sort((left, right) => right.time - left.time)
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
  const orderType = params.type === 'MARKET_PRICE' ? 'market' : params.type === 'STOP_LIMIT' ? 'stop_limit' : 'limit'
  const side = params.direction === 'SELL' ? 'sell' : 'buy'
  const referencePrice = params.price ?? 0
  const request: BackendCreateSpotOrderRequest = {
    pair_id: marketPairId(params.symbol),
    side,
    order_type: orderType,
    quantity: String(params.amount),
    idempotency_key: idempotencyKey,
  }

  if (orderType === 'limit') {
    request.price = String(params.price ?? 0)
  } else if (orderType === 'stop_limit') {
    request.price = String(params.price ?? 0)
    request.trigger_price = String(params.triggerPrice ?? 0)
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

export function mapPcWithdrawalRequest(params: PcWithdrawalParams): BackendCreateWithdrawalRequest {
  return {
    asset_symbol: params.unit.trim().toUpperCase(),
    network: params.network?.trim() || undefined,
    address: params.address.trim(),
    amount: String(params.amount),
    fee: String(params.fee),
    fund_password: params.fundPassword?.trim() || undefined,
    totp_code: params.totpCode?.trim() || undefined,
  }
}

export function normalizeWalletLedgerPage(
  response: BackendWalletLedgerResponse,
  params: { pageNo: number; pageSize: number },
): PcApiResponse<PcPageData<PcTransactionRecord>> {
  const fallbackPageNumber = Math.max(params.pageNo - 1, 0)
  const fallbackPageSize = Math.max(params.pageSize, 1)
  const content = response.entries.map((entry) => ({
    id: entry.id,
    memberId: entry.user_id,
    amount: toNumber(entry.amount),
    fee: toNumber(entry.fee ?? 0),
    symbol: entry.symbol,
    type: entry.change_type,
    createTime: formatUnixMillis(entry.created_at),
    status: 1,
  }))
  const page = response.page
  const pageNumber = firstNumber(page?.number) ?? fallbackPageNumber
  const pageSize = firstNumber(page?.size) ?? fallbackPageSize
  const totalElements = firstNumber(page?.total_elements, page?.totalElements) ?? content.length
  const totalPages = firstNumber(page?.total_pages, page?.totalPages)
    ?? Math.max(Math.ceil(totalElements / fallbackPageSize), 1)

  return {
    code: 0,
    message: 'success',
    data: {
      content,
      page: {
        number: Math.max(Math.floor(pageNumber), 0),
        size: Math.max(Math.floor(pageSize), 1),
        totalElements: Math.max(Math.floor(totalElements), 0),
        totalPages: Math.max(Math.floor(totalPages), 1),
      },
    },
  }
}

export function mapConvertPairsToPcCoins(response: BackendConvertPairsResponse): PcApiResponse<PcSwapCoinRow[]> {
  return {
    code: 0,
    message: 'success',
    data: mapConvertPairsToPcPairOptions(response).data.map((row) => ({
      id: row.id,
      fromUnit: row.fromUnit,
      toUnit: row.toUnit,
      minAmount: row.minAmount,
      maxAmount: row.maxAmount,
      feeRate: row.feeRate,
      enabled: row.enabled,
    })),
  }
}

export function mapConvertPairsToPcPairOptions(response: BackendConvertPairsResponse): PcApiResponse<PcSwapPairOption[]> {
  return {
    code: 0,
    message: 'success',
    data: response.pairs.flatMap((pair) => directionalConvertPairRows(pair)),
  }
}

export function mapConvertQuoteToPcQuote(quote: BackendConvertQuote): PcSwapQuote {
  return {
    quoteId: quote.quote_id,
    pairId: quote.convert_pair_id,
    fromAssetId: quote.from_asset_id,
    toAssetId: quote.to_asset_id,
    fromAmount: toNumber(quote.from_amount),
    toAmount: toNumber(quote.to_amount),
    rate: toNumber(quote.rate),
    spreadRate: toNumber(quote.spread_rate),
    feeRate: toNumber(quote.fee_rate),
    feeAmount: toNumber(quote.fee_amount),
    expiresAt: Number(quote.expires_at ?? 0),
  }
}

export function mapConvertOrdersToPcRows(
  response: BackendConvertOrdersResponse,
  context: {
    accounts?: BackendWalletAccountsResponse
    pairs?: BackendConvertPairsResponse
  } = {},
): PcApiResponse<PcSwapOrderRow[]> {
  const symbolByAssetId = convertAssetSymbolMap(context.pairs?.pairs ?? [], context.accounts?.accounts ?? [])
  return {
    code: 0,
    message: 'success',
    data: response.orders.map((order) => ({
      id: order.id,
      quoteId: order.quote_id,
      pairId: order.convert_pair_id,
      fromAssetId: order.from_asset_id,
      toAssetId: order.to_asset_id,
      fromUnit: symbolByAssetId.get(order.from_asset_id) ?? String(order.from_asset_id),
      toUnit: symbolByAssetId.get(order.to_asset_id) ?? String(order.to_asset_id),
      fromAmount: toNumber(order.from_amount),
      toAmount: toNumber(order.to_amount),
      rate: toNumber(order.rate),
      feeRate: toNumber(order.fee_rate ?? 0),
      feeAmount: toNumber(order.fee_amount ?? 0),
      status: order.status,
      createdAt: Number(order.created_at ?? 0),
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

export function mapSecondsProductsToPcCycles(response: BackendSecondsProductsResponse): PcApiResponse<Array<{ id: number; productId: number; symbol: string; cycleLength: number; cycleRate: number; minAmount: number; maxAmount: number }>> {
  return {
    code: 0,
    message: 'success',
    data: response.products.flatMap((product) => {
      const cycles = product.cycles?.length
        ? product.cycles
        : [
            {
              id: product.id,
              product_id: product.id,
              duration_seconds: product.duration_seconds,
              payout_rate: product.payout_rate,
              min_stake: product.min_stake,
              max_stake: product.max_stake,
            },
          ]
      return cycles.map((cycle) => ({
        id: cycle.id || product.id,
        productId: cycle.product_id || product.id,
        symbol: displaySymbolFromCompact(product.symbol),
        cycleLength: cycle.duration_seconds,
        cycleRate: toNumber(cycle.payout_rate),
        minAmount: toNumber(cycle.min_stake),
        maxAmount: toNumber(cycle.max_stake ?? 0),
      }))
    }),
  }
}

export function mapPcSecondsOrderRequest(
  params: { cycleId: number; productId?: number; durationSeconds?: number; direction: 0 | 1; amount: number },
  idempotencyKey: string,
): BackendCreateSecondsOrderRequest {
  const request: BackendCreateSecondsOrderRequest = {
    product_id: params.productId ?? params.cycleId,
    direction: params.direction === 1 ? 'down' : 'up',
    stake_amount: String(params.amount),
    idempotency_key: idempotencyKey,
  }
  if (params.durationSeconds) {
    request.duration_seconds = params.durationSeconds
  }
  return request
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
      cycleLength: toNumber(order.duration_seconds ?? 0),
      cycleRate: toNumber(order.payout_rate),
      status: secondsStatusToPc(order.status),
      result: secondsResultToPc(order.result),
      profit: secondsProfit(order),
      createTime: Number(order.created_at ?? order.opened_at ?? order.time ?? 0),
      endTime: Number(order.expires_at ?? 0),
    })),
  }
}

export function mapMarginProductsToContractCoins(response: BackendMarginProductsResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: response.products.map((product) => {
      const marginModes = resolveMarginModes(response.capabilities?.margin_modes, product.margin_modes, product.margin_mode)
      return {
      id: product.id,
      symbol: displaySymbolFromCompact(product.symbol),
      baseSymbol: product.margin_asset_symbol,
      coinSymbol: displaySymbolFromCompact(product.symbol).split('/')[0],
      logoUrl: typeof product.logo_url === 'string' ? product.logo_url.trim() : '',
      enable: product.status === 'active',
      sort: product.id,
      minTurnover: toNumber(product.min_margin),
      maxLeverage: toNumber(product.max_leverage),
      marginRate: toNumber(product.maintenance_margin_rate),
      marginModes,
      usdtPattern: marginModes.includes('cross') ? 'CROSSED' : 'FIXED',
      leverage: parseLeverageLevels(product.leverage_levels),
      }
    }),
  }
}

export function mapPcMarginOpenRequest(
  params: { contractCoinId: number; direction: 0 | 1; leverage: number; marginMode?: 'cross' | 'isolated'; volume: number },
  idempotencyKey: string,
): BackendCreateMarginPositionRequest {
  const request: BackendCreateMarginPositionRequest = {
    product_id: params.contractCoinId,
    direction: params.direction === 1 ? 'short' : 'long',
    order_type: 'market',
    margin_amount: String(params.volume),
    leverage: String(params.leverage),
    idempotency_key: idempotencyKey,
  }
  if (params.marginMode) request.margin_mode = params.marginMode
  return request
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

export function mapMarginWalletsToContractWallets(response: BackendMarginWalletsResponse): PcApiResponse<unknown[]> {
  return {
    code: 0,
    message: 'success',
    data: [
      ...response.wallets.map((wallet) => ({
        id: wallet.asset_id,
        memberId: 0,
        symbol: wallet.asset_symbol,
        coinSymbol: wallet.asset_symbol,
        baseSymbol: wallet.asset_symbol,
        usdtBalance: toNumber(wallet.available),
        usdtFrozenBalance: toNumber(wallet.frozen),
        usdtBuyPosition: 0,
        usdtBuyPrice: 0,
        usdtBuyLeverage: 1,
        usdtBuyPrincipalAmount: 0,
        usdtFrozenBuyPosition: 0,
        usdtSellPosition: 0,
        usdtSellPrice: 0,
        usdtSellLeverage: 1,
        usdtSellPrincipalAmount: 0,
        usdtFrozenSellPosition: 0,
        usdtShareNumber: 1,
        usdtPattern: 'FIXED',
        usdtTotalProfitAndLoss: 0,
        currentPrice: 0,
        closeFee: 0,
        maintenanceMarginRate: 0,
      })),
      ...response.positions.map(mapMarginPositionToContractWallet),
    ],
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

export function mapPublicNewsItemsToPcNewsCards(response: BackendPublicNewsItemsResponse, locale?: string): PcNewsCard[] {
  return response.news.map((item) => {
    const content = selectNewsContent(item, locale)
    const contentText = newsContentToPlainText(content.content)
    const contentHtml = newsContentToHtml(content.content)
    const summaryText = newsContentToPlainText(content.summary)
    const summary = summaryText || contentText.slice(0, 180)
    return {
      id: item.id,
      title: content.title || item.title,
      summary,
      content: contentHtml || escapeHtml(summary || ''),
      category: normalizeBackendNewsCategory(item.category),
      time: formatUnixMillis(item.published_at || item.updated_at || item.created_at),
      source: 'MarketEx Team',
      bannerUrl: item.banner_url?.trim() || undefined,
      smallLogoUrl: item.small_logo_url?.trim() || undefined,
    }
  })
}

function stripHtml(value: string): string {
  return value.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim()
}

function newsContentToPlainText(value: BackendNewsRichTextValue): string {
  if (!value) return ''
  if (typeof value === 'string') return stripHtml(value)
  return value
    .map((block) => (block.children ?? []).map((leaf) => leaf.text).join(''))
    .join('\n')
    .replace(/\s+/g, ' ')
    .trim()
}

function newsContentToHtml(value: BackendNewsRichTextValue): string {
  if (!value) return ''
  if (typeof value === 'string') return escapeHtml(stripHtml(value))
  return value.map(richTextBlockToHtml).join('')
}

function richTextBlockToHtml(block: BackendNewsRichTextBlock): string {
  if (block.type === 'image') {
    return richTextImageBlockToHtml(block)
  }

  const tag = newsRichTextBlockTag(block.type)
  const content = (block.children ?? []).map(richTextLeafToHtml).join('') || '<br>'
  return `<${tag}>${content}</${tag}>`
}

function richTextImageBlockToHtml(block: BackendNewsRichTextBlock): string {
  const url = block.url?.trim()
  if (!url) return ''
  const alt = block.alt?.trim() || ''
  return `<figure><img src="${escapeHtml(url)}" alt="${escapeHtml(alt)}" /></figure>`
}

function richTextLeafToHtml(leaf: BackendNewsRichTextLeaf): string {
  let content = escapeHtml(leaf.text)
  if (leaf.underline) content = `<u>${content}</u>`
  if (leaf.italic) content = `<em>${content}</em>`
  if (leaf.bold) content = `<strong>${content}</strong>`
  return content
}

function newsRichTextBlockTag(type: string): 'p' | 'h1' | 'h2' | 'h3' | 'blockquote' {
  return type === 'h1' || type === 'h2' || type === 'h3' || type === 'blockquote' ? type : 'p'
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

function projectedEarnAmount(item: BackendEarnSubscription): number {
  return toNumber(item.amount) * toNumber(item.apr_rate) * (item.term_days / 365)
}

function mapEarnSubscriptionToPcOrder(item: BackendEarnSubscription): PcFinanceOrder {
  const subscribedAt = Number(item.subscribed_at ?? 0)
  return {
    id: item.id,
    orderNo: formatBusinessOrderNo('EA', { ...item, orderNo: item.order_no, createTime: subscribedAt }),
    memberId: item.user_id,
    financeId: item.product_id,
    coinSymbol: item.asset_symbol || String(item.asset_id),
    num: toNumber(item.amount),
    status: financeSubscriptionStatusToPc(item.status),
    earnNum: projectedEarnAmount(item),
    hourCursor: 0,
    cycle: item.term_days,
    breachFee: toNumber(item.early_redeem_fee_rate ?? 0),
    minDaysProfit: toNumber(item.apr_rate),
    maxDaysProfit: toNumber(item.apr_rate),
    createTime: subscribedAt,
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

function parseMarginModes(modes: string[] | string | undefined, fallbackMode: string): Array<'cross' | 'isolated'> {
  const values = Array.isArray(modes) ? modes : typeof modes === 'string' ? modes.split(',') : [fallbackMode]
  const normalized = values
    .map((mode) => mode.trim().toLowerCase())
    .filter((mode): mode is 'cross' | 'isolated' => mode === 'cross' || mode === 'isolated')
  return normalized.length > 0 ? [...new Set(normalized)] : ['isolated']
}

function resolveMarginModes(
  capabilityModes: string[] | string | undefined,
  productModes: string[] | string | undefined,
  fallbackMode: string,
): Array<'cross' | 'isolated'> {
  const configured = parseMarginModes(productModes, fallbackMode)
  if (!capabilityModes) return configured
  const supported = parseMarginModes(capabilityModes, 'isolated')
  const usable = configured.filter((mode) => supported.includes(mode))
  return usable.length > 0 ? usable : supported
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
  const type = order.order_type === 'market'
    ? 'MARKET_PRICE'
    : order.order_type === 'stop_limit'
      ? 'STOP_LIMIT'
      : 'LIMIT_PRICE'
  return {
    orderId: order.id,
    symbol: displaySymbolFromCompact(order.pair_id),
    direction: order.side === 'sell' ? 'SELL' : 'BUY',
    type,
    price: toNumber(order.price ?? 0),
    triggerPrice: firstNumber(order.trigger_price) ?? null,
    amount: toNumber(order.quantity),
    filledAmount: toNumber(order.filled_quantity),
    filledPrice: firstNumber(order.average_price) ?? null,
    status: spotOrderStatusToPc(order.status),
    time: Number(order.created_at ?? 0),
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

function directionalConvertPairRows(pair: BackendConvertPair): PcSwapPairOption[] {
  const fromUnit = (pair.from_asset_symbol || String(pair.from_asset_id)).toUpperCase()
  const toUnit = (pair.to_asset_symbol || String(pair.to_asset_id)).toUpperCase()
  const sourceLimits = {
    id: pair.id,
    minAmount: toNumber(pair.min_amount),
    maxAmount: toNumber(pair.max_amount ?? 0),
    feeRate: toNumber(pair.fee_rate),
    enabled: pair.enabled,
  }
  const targetLimits = {
    id: pair.id,
    minAmount: toNumber(pair.target_min_amount ?? pair.min_amount),
    maxAmount: toNumber(pair.target_max_amount ?? pair.max_amount ?? 0),
    feeRate: toNumber(pair.fee_rate),
    enabled: pair.enabled,
  }
  return [
    {
      ...sourceLimits,
      fromAssetId: pair.from_asset_id,
      toAssetId: pair.to_asset_id,
      fromUnit,
      toUnit,
    },
    {
      ...targetLimits,
      fromAssetId: pair.to_asset_id,
      toAssetId: pair.from_asset_id,
      fromUnit: toUnit,
      toUnit: fromUnit,
    },
  ]
}

function convertAssetSymbolMap(
  pairs: BackendConvertPair[],
  accounts: BackendWalletAccount[],
): Map<number, string> {
  const symbols = new Map<number, string>()
  for (const pair of pairs) {
    if (pair.from_asset_symbol) symbols.set(pair.from_asset_id, pair.from_asset_symbol.toUpperCase())
    if (pair.to_asset_symbol) symbols.set(pair.to_asset_id, pair.to_asset_symbol.toUpperCase())
  }
  for (const account of accounts) {
    symbols.set(account.asset_id, account.symbol.toUpperCase())
  }
  return symbols
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

function maskInvitee(value: string): string {
  if (value.includes('@')) {
    const [name, domain] = value.split('@')
    return `${name.slice(0, 1)}***@${domain}`
  }
  if (value.length <= 4) return value
  return `${value.slice(0, 3)}****${value.slice(-2)}`
}

function normalizePcLocale(value?: string | null): PcLocale | undefined {
  const locale = value?.trim().toLowerCase()
  if (!locale) return undefined
  if (locale === 'zh' || locale.startsWith('zh-')) return 'zh'
  if (locale === 'en' || locale.startsWith('en-')) return 'en'
  return undefined
}

function normalizeSupportedLocales(values?: string[] | null): PcLocale[] {
  const locales: PcLocale[] = []
  for (const value of values ?? []) {
    const locale = normalizePcLocale(value)
    if (locale && !locales.includes(locale)) {
      locales.push(locale)
    }
  }
  return locales
}

function fallbackSupportedLocale(fallback: PcLocale, supportedLocales: PcLocale[]): PcLocale {
  if (supportedLocales.length === 0 || supportedLocales.includes(fallback)) return fallback
  return supportedLocales[0] || 'en'
}

function selectNewsContent(item: BackendPublicNewsItem, locale?: string): BackendNewsContentTranslation {
  const items = item.content_json?.items ?? []
  const currentLocale = normalizePcLocale(locale)
  const defaultLocale = normalizePcLocale(item.default_locale || item.content_json?.default_locale)
  return (
    (currentLocale ? items.find((entry) => normalizePcLocale(entry.locale) === currentLocale) : undefined) ||
    (defaultLocale ? items.find((entry) => normalizePcLocale(entry.locale) === defaultLocale) : undefined) ||
    items[0] ||
    { locale: currentLocale || defaultLocale || 'en' }
  )
}

function normalizeBackendNewsCategory(category: string): string {
  const normalized = category.toLowerCase()
  if (['general', 'market', 'product', 'system', 'promotion'].includes(normalized)) return normalized
  return 'general'
}

function firstNumber(...values: Array<string | number | null | undefined>): number | undefined {
  const value = values.find((item) => item !== undefined && item !== null && item !== '')
  return value === undefined ? undefined : toNumber(value)
}

function toNumber(value: string | number | null | undefined): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function formatUnixMillis(value: number): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (part: number) => String(part).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}
