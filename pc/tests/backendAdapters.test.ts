import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { APP_CONFIG } from '../src/config/app.ts'
import {
  backendApiUrl,
  createAuthorizationHeader,
  mapMarketDepthToTradePlate,
  mapMarketKlinesToPcRows,
  mapMarketTickerToPcTicker,
  mapMarketTradeToPcTrade,
  mapMarketsToPcTickers,
  mapConvertOrdersToPcRows,
  mapConvertPairsToPcCoins,
  mapConvertPairsToPcPairOptions,
  mapConvertQuoteToPcQuote,
  mapEarnProductsToPcFinanceList,
  mapEarnSubscriptionsToPcFinanceCount,
  mapEarnSubscriptionsToPcFinancePage,
  mapEarnSubscriptionsToPcFinanceStatistic,
  mapMarginPositionsToContractOrders,
  mapMarginPositionsToContractWallets,
  mapMarginProductsToContractCoins,
  mapMyInvitesToPcInviteRecords,
  mapNewCoinProjectsToPcActivityPage,
  mapPublicCountriesToPcOptions,
  mapPublicNewsItemsToPcNewsCards,
  mapPcMarginOpenRequest,
  mapPcWithdrawalRequest,
  resolveProfileLocale,
  mapPcNewCoinSubscriptionRequest,
  mapPcSecondsOrderRequest,
  mapPcSpotOrderRequest,
  mapSecondsOrdersToPcOrders,
  mapSecondsProductsToPcCycles,
  mapSecondsProductsToPcTickers,
  mapSpotOrdersToPcPage,
  mapWalletAccountsToMemberWallets,
  mapWalletAccountsToTradeWallets,
  normalizeAuthResponse,
  normalizeProfileForSecurity,
  normalizeWalletLedgerPage,
} from '../src/api/backendAdapters.ts'

test('normalizes backend auth tokens into the current PC login response shape', () => {
  const response = normalizeAuthResponse({
    access_token: 'access-token',
    refresh_token: 'refresh-token',
    token_type: 'Bearer',
    scope: 'user',
  })

  assert.equal(response.code, 0)
  assert.equal(response.data.token, 'access-token')
  assert.equal(response.data.accessToken, 'access-token')
  assert.equal(response.data.refreshToken, 'refresh-token')
  assert.equal(response.data.tokenType, 'Bearer')
  assert.equal(response.data.scope, 'user')
})

test('normalizes backend login 2FA challenges without creating a token session', () => {
  const challenge = normalizeAuthResponse({
    requires_2fa: true,
    challenge_id: 'challenge-1',
    expires_in_seconds: 300,
  })

  assert.equal(challenge.code, 0)
  assert.equal(challenge.data.requires2fa, true)
  assert.equal(challenge.data.challengeId, 'challenge-1')
  assert.equal(challenge.data.expiresInSeconds, 300)
  assert.equal(Object.hasOwn(challenge.data, 'token'), false)

  const setupChallenge = normalizeAuthResponse({
    requires_2fa_setup: true,
    setup_challenge_id: 'setup-1',
    expires_in_seconds: 300,
  })

  assert.equal(setupChallenge.code, 0)
  assert.equal(setupChallenge.data.requires2faSetup, true)
  assert.equal(setupChallenge.data.setupChallengeId, 'setup-1')
  assert.equal(Object.hasOwn(setupChallenge.data, 'token'), false)
})

test('uses only the new backend API base URL configuration', () => {
  assert.equal(Object.hasOwn(APP_CONFIG, 'API_DOMAIN'), false)
  assert.equal(backendApiUrl('/auth/login'), 'http://127.0.0.1:8080/api/v1/auth/login')
  assert.equal(backendApiUrl('wallet/accounts'), 'http://127.0.0.1:8080/api/v1/wallet/accounts')
})

test('builds the new backend bearer authorization header', () => {
  assert.equal(createAuthorizationHeader('abc123'), 'Bearer abc123')
})

test('maps backend market list and ticker payloads into PC ticker rows', () => {
  const tickers = mapMarketsToPcTickers(
    {
      markets: [
        {
          symbol: 'BTC-USDT',
          logo_url: 'https://cdn.example.com/btc-usdt.png',
          base_asset: 'BTC',
          quote_asset: 'USDT',
          price_precision: 8,
          qty_precision: 8,
          min_order_value: '1',
          status: 'active',
          market_type: 'external',
        },
      ],
    },
    {
      BTCUSDT: {
        symbol: 'BTCUSDT',
        last_price: '70001.12',
        high_24h: '71000',
        low_24h: '69000',
        volume_24h: '245.5',
        price_change_24h: '1001.12',
        price_change_percent_24h: '1.45',
        observed_at: 1_717_171_000_000,
      },
    },
  )

  assert.equal(tickers[0].symbol, 'BTC/USDT')
  assert.equal(tickers[0].icon, 'https://cdn.example.com/btc-usdt.png')
  assert.equal(tickers[0].close, 70001.12)
  assert.equal(tickers[0].open, 69000)
  assert.equal(tickers[0].high, 71000)
  assert.equal(tickers[0].low, 69000)
  assert.equal(tickers[0].volume, 245.5)
  assert.equal(tickers[0].chg, 1.45)
  assert.equal(tickers[0].time, 1_717_171_000_000)
  assert.equal(tickers[0].zone, 0)
  const updated = mapMarketTickerToPcTicker(tickers[0], {
    symbol: 'BTCUSDT',
    last_price: '70002',
    high_24h: '71100',
    low_24h: '68900',
    volume_24h: '250',
    price_change_24h: '1002',
    price_change_percent_24h: '1.4522',
    observed_at: 1_717_171_001_000,
  })
  assert.equal(updated.close, 70002)
  assert.equal(updated.open, 69000)
  assert.equal(updated.high, 71100)
  assert.equal(updated.low, 68900)
  assert.equal(updated.volume, 250)
  assert.equal(updated.turnover, 70002 * 250)
  assert.equal(updated.time, 1_717_171_001_000)
  assert.equal(updated.chg, 1.4522)
  assert.equal(updated.icon, 'https://cdn.example.com/btc-usdt.png')
})

test('maps backend market klines, depth, and trades into PC market shapes', () => {
  const klines = mapMarketKlinesToPcRows([
    {
      symbol: 'BTCUSDT',
      interval: '1m',
      open_time: 1_717_171_000_000,
      open: '70000',
      high: '70100',
      low: '69900',
      close: '70050',
      volume: '12.5',
    },
  ])
  assert.deepEqual(klines[0], [1_717_171_000_000, 70000, 70100, 69900, 70050, 12.5])

  const depth = mapMarketDepthToTradePlate({
    symbol: 'BTCUSDT',
    bids: [{ price: '70000', amount: '1.2' }],
    asks: [{ price: '70010', quantity: '0.8' }],
    observed_at: 1_717_171_000_000,
  })
  assert.deepEqual(depth.bids[0], { price: 70000, amount: 1.2 })
  assert.deepEqual(depth.asks[0], { price: 70010, amount: 0.8 })

  assert.deepEqual(mapMarketTradeToPcTrade({
    id: '9',
    symbol: 'BTCUSDT',
    side: 'sell',
    price: '70020',
    quantity: '0.3',
    traded_at: 1_717_171_002_000,
  }), {
    id: '9',
    symbol: 'BTCUSDT',
    direction: 'SELL',
    price: 70020,
    amount: 0.3,
    time: 1_717_171_002_000,
  })
})

test('normalizes backend profile into the current PC security shape with country locale metadata', () => {
  const response = normalizeProfileForSecurity({
    id: 7,
    username: 'moon_1024',
    email: 'user@example.com',
    phone: null,
    country_code: 'CN',
    preferred_locale: 'zh',
    default_locale: 'zh',
    supported_locales: ['zh', 'en'],
    status: 'active',
    kyc_level: 1,
    email_verified_at: 1_717_171_717_000,
    fund_password_set: true,
    created_at: 1_717_171_000_000,
  })

  assert.equal(response.code, 0)
  assert.equal(response.data.id, 7)
  assert.equal(response.data.email, 'user@example.com')
  assert.equal(response.data.username, 'moon_1024')
  assert.equal(response.data.emailVerified, 1)
  assert.equal(response.data.fundsVerified, 1)
  assert.equal(response.data.transactionStatus, 1)
  assert.equal(response.data.kycLevel, 1)
  assert.equal(response.data.countryCode, 'CN')
  assert.equal(response.data.preferredLocale, 'zh')
  assert.equal(response.data.defaultLocale, 'zh')
  assert.deepEqual(response.data.supportedLocales, ['zh', 'en'])
})

test('maps public country configs into PC registration options', () => {
  const countries = mapPublicCountriesToPcOptions({
    countries: [
      {
        country_code: 'CN',
        country_name: '中国',
        default_locale: 'zh',
        supported_locales: ['zh', 'en'],
      },
    ],
  })

  assert.deepEqual(countries, [
    {
      code: 'CN',
      name: '中国',
      defaultLocale: 'zh',
      supportedLocales: ['zh', 'en'],
    },
  ])
})

test('resolves profile locale with manual override priority and supported locale fallback', () => {
  const profile = {
    defaultLocale: 'zh',
    preferredLocale: 'zh',
    supportedLocales: ['zh'],
  }

  assert.equal(resolveProfileLocale(profile, 'en', false), 'zh')
  assert.equal(resolveProfileLocale(profile, 'en', true), 'zh')
  assert.equal(resolveProfileLocale({ ...profile, supportedLocales: ['zh', 'en'] }, 'en', true), 'en')
  assert.equal(resolveProfileLocale({ supportedLocales: [] }, 'en', false), 'en')
})

test('maps backend wallet accounts into existing PC wallet rows', () => {
  const response = mapWalletAccountsToMemberWallets({
    accounts: [
      {
        user_id: 7,
        asset_id: 2,
        symbol: 'USDT',
        logo_url: 'https://cdn.example.test/assets/usdt.png',
        available: '100.5',
        frozen: '2',
        locked: '3',
      },
    ],
  })

  assert.equal(response.code, 0)
  assert.equal(response.data[0].memberId, 7)
  assert.equal(response.data[0].coin.coinGroup, 'USDT')
  assert.equal(response.data[0].coin.logoUrl, 'https://cdn.example.test/assets/usdt.png')
  assert.equal(response.data[0].balance, 100.5)
  assert.equal(response.data[0].frozenBalance, 5)
})

test('maps backend spot order payloads into PC order history rows', () => {
  const page = mapSpotOrdersToPcPage(
    {
      orders: [
        {
          id: '91',
          user_id: '7',
          pair_id: 'BTC-USDT',
          side: 'buy',
          order_type: 'limit',
          price: '70000',
          quantity: '0.2',
          filled_quantity: '0.05',
          average_price: '70500',
          status: 'open',
          created_at: 1_717_171_123_000,
        },
        {
          id: '92',
          user_id: '7',
          pair_id: 'BTC-USDT',
          side: 'sell',
          order_type: 'market',
          price: null,
          quantity: '0.1',
          filled_quantity: '0.1',
          average_price: '71000',
          status: 'filled',
          created_at: 1_717_171_999_000,
        },
        {
          id: '90',
          user_id: '7',
          pair_id: 'BTC-USDT',
          side: 'sell',
          order_type: 'stop_limit',
          price: '69000',
          trigger_price: '69500',
          quantity: '0.3',
          filled_quantity: '0',
          average_price: null,
          status: 'pending',
          created_at: 1_717_171_000_000,
        },
      ],
    },
    { pageNo: 0, pageSize: 10 },
  )

  assert.equal(page.code, 0)
  assert.deepEqual(page.data.content.map((order) => order.orderId), ['92', '91', '90'])
  assert.equal(page.data.content[1].orderId, '91')
  assert.equal(page.data.content[1].symbol, 'BTC/USDT')
  assert.equal(page.data.content[1].direction, 'BUY')
  assert.equal(page.data.content[1].type, 'LIMIT_PRICE')
  assert.equal(page.data.content[1].price, 70000)
  assert.equal(page.data.content[1].amount, 0.2)
  assert.equal(page.data.content[1].filledAmount, 0.05)
  assert.equal(page.data.content[1].filledPrice, 70500)
  assert.equal(page.data.content[1].status, 'TRADING')
  assert.equal(page.data.content[1].time, 1_717_171_123_000)
  assert.equal(page.data.content[2].type, 'STOP_LIMIT')
  assert.equal(page.data.content[2].triggerPrice, 69500)
  assert.equal(page.data.content[2].price, 69000)
})

test('maps PC spot order requests and trade wallet balances to backend shapes', () => {
  assert.deepEqual(mapPcSpotOrderRequest({
    symbol: 'BTC/USDT',
    direction: 'BUY',
    type: 'LIMIT_PRICE',
    price: 70000,
    amount: 0.2,
  }, 'spot-request-1'), {
    pair_id: 'BTC-USDT',
    side: 'buy',
    order_type: 'limit',
    price: '70000',
    quantity: '0.2',
    idempotency_key: 'spot-request-1',
  })

  assert.deepEqual(mapPcSpotOrderRequest({
    symbol: 'ETH/USDT',
    direction: 'SELL',
    type: 'MARKET_PRICE',
    amount: 3,
    price: 2500,
  }, 'spot-request-2'), {
    pair_id: 'ETH-USDT',
    side: 'sell',
    order_type: 'market',
    quantity: '3',
    reference_price: '2500',
    idempotency_key: 'spot-request-2',
  })

  assert.deepEqual(mapPcSpotOrderRequest({
    symbol: 'ETH/USDT',
    direction: 'BUY',
    type: 'MARKET_PRICE',
    amount: 2,
    price: 2500,
  }, 'spot-request-3'), {
    pair_id: 'ETH-USDT',
    side: 'buy',
    order_type: 'market',
    quantity: '2',
    reference_price: '2500',
    idempotency_key: 'spot-request-3',
  })

  assert.deepEqual(mapPcSpotOrderRequest({
    symbol: 'BTC/USDT',
    direction: 'SELL',
    type: 'STOP_LIMIT',
    triggerPrice: 69500,
    price: 69000,
    amount: 0.3,
  }, 'spot-request-4'), {
    pair_id: 'BTC-USDT',
    side: 'sell',
    order_type: 'stop_limit',
    price: '69000',
    trigger_price: '69500',
    quantity: '0.3',
    idempotency_key: 'spot-request-4',
  })

  const wallets = mapWalletAccountsToTradeWallets({
    accounts: [
      { user_id: 7, asset_id: 1, symbol: 'BTC', available: '0.4', frozen: '0', locked: '0' },
      { user_id: 7, asset_id: 2, symbol: 'USDT', available: '1000', frozen: '0', locked: '0' },
    ],
  }, 'BTC/USDT')
  assert.deepEqual(wallets, [
    { symbol: 'BTC', balance: 0.4, frozenBalance: 0 },
    { symbol: 'USDT', balance: 1000, frozenBalance: 0 },
  ])
})

test('maps PC withdrawal request into backend security verification payload', () => {
  assert.deepEqual(mapPcWithdrawalRequest({
    unit: 'usdt',
    network: 'TRC20',
    address: 'TDestinationAddress',
    amount: 25.5,
    fee: 1,
    code: 'legacy-email-code',
    fundPassword: 'fund-secret',
    totpCode: '123456',
  }), {
    asset_symbol: 'USDT',
    network: 'TRC20',
    address: 'TDestinationAddress',
    amount: '25.5',
    fee: '1',
    fund_password: 'fund-secret',
    totp_code: '123456',
  })

  assert.deepEqual(mapPcWithdrawalRequest({
    unit: 'btc',
    address: 'bc1destination',
    amount: 0.1,
    fee: 0,
    code: '',
  }), {
    asset_symbol: 'BTC',
    network: undefined,
    address: 'bc1destination',
    amount: '0.1',
    fee: '0',
    fund_password: undefined,
    totp_code: undefined,
  })
})

test('maps backend wallet ledger into the current transaction history page shape', () => {
  const response = normalizeWalletLedgerPage(
    {
      entries: [
        {
          id: 11,
          user_id: 7,
          asset_id: 2,
          symbol: 'USDT',
          change_type: 'admin_recharge',
          amount: '10.25',
          balance_type: 'available',
          balance_after: '110.25',
          available_after: '110.25',
          frozen_after: '0',
          locked_after: '0',
          fee: '0.25',
          ref_type: 'admin_recharge',
          ref_id: 'r1',
          created_at: 1_717_171_000_000,
        },
        {
          id: 12,
          user_id: 7,
          asset_id: 2,
          symbol: 'USDT',
          change_type: 'quick_recharge',
          amount: '50',
          balance_type: 'available',
          balance_after: '160.25',
          available_after: '160.25',
          frozen_after: '0',
          locked_after: '0',
          fee: '0',
          ref_type: 'quick_recharge',
          ref_id: 'qr1',
          created_at: 1_717_171_100_000,
        },
        {
          id: 13,
          user_id: 7,
          asset_id: 2,
          symbol: 'USDT',
          change_type: 'convert_settlement',
          amount: '-12.5',
          balance_type: 'available',
          balance_after: '147.75',
          available_after: '147.75',
          frozen_after: '0',
          locked_after: '0',
          fee: '0.1',
          ref_type: 'convert_order',
          ref_id: 'co1',
          created_at: 1_717_171_200_000,
        },
      ],
      page: {
        number: 2,
        size: 10,
        total_elements: 123,
        total_pages: 13,
      },
    },
    { pageNo: 1, pageSize: 10 },
  )

  assert.equal(response.code, 0)
  assert.equal(response.data.content[0].id, 11)
  assert.equal(response.data.content[0].memberId, 7)
  assert.equal(response.data.content[0].symbol, 'USDT')
  assert.equal(response.data.content[0].amount, 10.25)
  assert.equal(response.data.content[0].fee, 0.25)
  assert.equal(response.data.content[0].type, 'admin_recharge')
  assert.equal(response.data.content[1].type, 'quick_recharge')
  assert.equal(response.data.content[2].amount, -12.5)
  assert.equal(response.data.content[2].fee, 0.1)
  assert.equal(response.data.content[2].type, 'convert_settlement')
  assert.equal(response.data.content[0].status, 1)
  assert.equal(response.data.page.number, 2)
  assert.equal(response.data.page.size, 10)
  assert.equal(response.data.page.totalElements, 123)
  assert.equal(response.data.page.totalPages, 13)
})

test('maps backend convert pairs into PC swap coin rows', () => {
  const rows = mapConvertPairsToPcCoins({
    pairs: [
      {
        id: 8,
        from_asset_id: 1,
        from_asset_symbol: 'ETH',
        to_asset_id: 2,
        to_asset_symbol: 'USDT',
        pricing_mode: 'market',
        spread_rate: '0.001',
        fee_rate: '0.002',
        min_amount: '0.01',
        max_amount: '10',
        target_min_amount: '100',
        target_max_amount: '1000',
        enabled: true,
      },
    ],
  })

  assert.equal(rows.code, 0)
  assert.deepEqual(rows.data, [
    { id: 8, fromUnit: 'ETH', toUnit: 'USDT', minAmount: 0.01, maxAmount: 10, feeRate: 0.002, enabled: true },
    { id: 8, fromUnit: 'USDT', toUnit: 'ETH', minAmount: 100, maxAmount: 1000, feeRate: 0.002, enabled: true },
  ])

  const options = mapConvertPairsToPcPairOptions({
    pairs: [
      {
        id: 8,
        from_asset_id: 1,
        from_asset_symbol: 'ETH',
        to_asset_id: 2,
        to_asset_symbol: 'USDT',
        pricing_mode: 'market',
        spread_rate: '0.001',
        fee_rate: '0.002',
        min_amount: '0.01',
        max_amount: '10',
        target_min_amount: '100',
        target_max_amount: '1000',
        enabled: true,
      },
    ],
  })
  assert.equal(options.data[0].fromAssetId, 1)
  assert.equal(options.data[0].toAssetId, 2)
  assert.equal(options.data[0].minAmount, 0.01)
  assert.equal(options.data[0].maxAmount, 10)
  assert.equal(options.data[0].feeRate, 0.002)
  assert.equal(options.data[1].fromAssetId, 2)
  assert.equal(options.data[1].toAssetId, 1)
  assert.equal(options.data[1].minAmount, 100)
  assert.equal(options.data[1].maxAmount, 1000)
  assert.equal(options.data[1].feeRate, 0.002)
})

test('maps backend convert quote fee snapshot', () => {
  const quote = mapConvertQuoteToPcQuote({
    quote_id: 'quote-fee',
    convert_pair_id: 8,
    from_asset_id: 1,
    to_asset_id: 2,
    from_amount: '10',
    to_amount: '19.8',
    rate: '2',
    spread_rate: '0',
    fee_rate: '0.01',
    fee_amount: '0.1',
    expires_at: 1_717_171_030_000,
  })

  assert.equal(quote.quoteId, 'quote-fee')
  assert.equal(quote.feeRate, 0.01)
  assert.equal(quote.feeAmount, 0.1)
  assert.equal(quote.toAmount, 19.8)
})

test('maps backend convert orders into PC swap rows with symbols', () => {
  const rows = mapConvertOrdersToPcRows(
    {
      orders: [
        {
          id: 31,
          quote_id: 'quote-31',
          convert_pair_id: 8,
          from_asset_id: 2,
          to_asset_id: 1,
          from_amount: '100',
          to_amount: '0.5',
          rate: '0.005',
          fee_rate: '0.001',
          fee_amount: '0.1',
          status: 'completed',
          created_at: 1_717_171_000_000,
        },
      ],
    },
    {
      pairs: {
        pairs: [
          {
            id: 8,
            from_asset_id: 1,
            from_asset_symbol: 'ETH',
            to_asset_id: 2,
            to_asset_symbol: 'USDT',
            pricing_mode: 'fixed',
            spread_rate: '0',
            fee_rate: '0.001',
            min_amount: '1',
            max_amount: null,
            enabled: true,
          },
        ],
      },
    },
  )

  assert.equal(rows.code, 0)
  assert.equal(rows.data[0].quoteId, 'quote-31')
  assert.equal(rows.data[0].fromUnit, 'USDT')
  assert.equal(rows.data[0].toUnit, 'ETH')
  assert.equal(rows.data[0].fromAmount, 100)
  assert.equal(rows.data[0].toAmount, 0.5)
  assert.equal(rows.data[0].feeRate, 0.001)
  assert.equal(rows.data[0].feeAmount, 0.1)
})

test('maps backend earn products and subscriptions into PC finance shapes', () => {
  const products = mapEarnProductsToPcFinanceList({
    products: [
      {
        id: 3,
        asset_id: 2,
        asset_symbol: 'USDT',
        name: '30 Days USDT',
        category: 'fixed',
        introduction_json: {},
        term_days: 30,
        apr_rate: '0.12',
        min_subscribe: '100',
        max_subscribe: '10000',
        status: 'active',
      },
    ],
  })
  assert.equal(products.data[0].acceptUnit, 'USDT')
  assert.equal(products.data[0].cycle, 30)
  assert.equal(products.data[0].minDaysProfit, 0.12)
  assert.equal(products.data[0].step, 1)

  const subscriptions = {
    subscriptions: [
      {
        id: 71,
        order_no: 'EA202606170001',
        user_id: 7,
        product_id: 3,
        asset_id: 2,
        asset_symbol: 'USDT',
        amount: '500',
        apr_rate: '0.12',
        term_days: 30,
        status: 'subscribed',
        idempotency_key: 'earn-1',
        subscribed_at: 1_717_170_000_000,
        matures_at: 1_717_171_000_000,
      },
      {
        id: 72,
        user_id: 7,
        product_id: 3,
        asset_id: 2,
        asset_symbol: 'USDT',
        amount: '100',
        apr_rate: '0.10',
        term_days: 10,
        status: 'redeemed',
        idempotency_key: 'earn-2',
        subscribed_at: 1_717_170_000_000,
        matures_at: 1_717_171_000_000,
      },
    ],
  }
  assert.ok(Math.abs(mapEarnSubscriptionsToPcFinanceStatistic(subscriptions, 'USDT').data.earnNum - 5.205479452054795) < 0.000001)
  assert.equal(mapEarnSubscriptionsToPcFinanceStatistic(subscriptions, 'USDT').data.num, 1)
  assert.equal(mapEarnSubscriptionsToPcFinanceCount(subscriptions).data, 500)

  const page = mapEarnSubscriptionsToPcFinancePage(subscriptions, { pageNo: 1, pageSize: 10, status: 0 })
  assert.equal(page.data.content.length, 1)
  assert.equal(page.data.content[0].id, 71)
  assert.equal(page.data.content[0].orderNo, 'EA202606170001')
  assert.equal(page.data.content[0].coinSymbol, 'USDT')
  assert.equal(page.data.content[0].status, 0)
  assert.equal(page.data.content[0].num, 500)
  assert.notEqual(page.data.content[0].orderNo, String(page.data.content[0].id))
  assert.equal(page.data.content[0].createTime, 1_717_170_000_000)
})

test('maps backend new coin projects and subscription request into PC launchpad shapes', () => {
  const page = mapNewCoinProjectsToPcActivityPage(
    {
      projects: [
        {
          id: 5,
          asset_id: 9,
          symbol: 'NEW',
          lifecycle_status: 'subscription',
          total_supply: '1000000',
          issue_price: '0.5',
          listed_at: 1_717_171_000_000,
          unlock_type: 'fixed',
          fixed_unlock_at: 1_717_171_000_000,
          relative_unlock_seconds: null,
          unlock_fee_enabled: false,
          unlock_fee_rate: null,
          unlock_fee_basis: null,
          unlock_fee_asset: null,
          status: 'active',
        },
      ],
    },
    { pageNo: 1, pageSize: 10, step: 1 },
  )

  assert.equal(page.code, 0)
  assert.equal(page.data.content[0].id, 5)
  assert.equal(page.data.content[0].unit, 'NEW')
  assert.equal(page.data.content[0].step, 1)
  assert.equal(page.data.content[0].price, 0.5)
  assert.equal(page.data.content[0].totalSupply, 1000000)

  assert.deepEqual(mapPcNewCoinSubscriptionRequest({ quoteAssetId: 2, amount: 100, price: 0.5 }, 'new-coin-1'), {
    quote_asset_id: 2,
    quote_amount: '100',
    quantity: '200',
    idempotency_key: 'new-coin-1',
  })
})

test('maps backend seconds contract products and orders into PC seconds shapes', () => {
  const cycles = mapSecondsProductsToPcCycles({
    products: [
      {
        id: 4,
        pair_id: 1,
        symbol: 'BTC/USDT',
        stake_asset: 2,
        stake_asset_symbol: 'USDT',
        duration_seconds: 60,
        payout_rate: '0.85',
        min_stake: '10',
        max_stake: '1000',
        status: 'active',
      },
    ],
  })
  assert.deepEqual(cycles.data, [{ id: 4, productId: 4, symbol: 'BTC/USDT', cycleLength: 60, cycleRate: 0.85, minAmount: 10, maxAmount: 1000 }])

  const multiCycles = mapSecondsProductsToPcCycles({
    products: [
      {
        id: 5,
        pair_id: 1,
        symbol: 'ETH-USDT',
        stake_asset: 2,
        stake_asset_symbol: 'USDT',
        duration_seconds: 60,
        payout_rate: '0.80',
        min_stake: '10',
        max_stake: null,
        cycles: [
          { id: 501, product_id: 5, duration_seconds: 60, payout_rate: '0.80', min_stake: '10', max_stake: null },
          { id: 502, product_id: 5, duration_seconds: 120, payout_rate: '0.90', min_stake: '20', max_stake: '2000' },
        ],
        status: 'active',
      },
    ],
  })
  assert.deepEqual(multiCycles.data, [
    { id: 501, productId: 5, symbol: 'ETH/USDT', cycleLength: 60, cycleRate: 0.8, minAmount: 10, maxAmount: 0 },
    { id: 502, productId: 5, symbol: 'ETH/USDT', cycleLength: 120, cycleRate: 0.9, minAmount: 20, maxAmount: 2000 },
  ])

  const secondsTickers = mapSecondsProductsToPcTickers(
    {
      products: [
        {
          id: 6,
          pair_id: 1,
          symbol: 'BTC/USDT',
          logo_url: 'https://cdn.example.com/btc.png',
          stake_asset: 2,
          stake_asset_symbol: 'USDT',
          duration_seconds: 60,
          payout_rate: '0.80',
          min_stake: '10',
          max_stake: null,
          status: 'active',
        },
        {
          id: 7,
          pair_id: 1,
          symbol: 'BTC-USDT',
          stake_asset: 2,
          stake_asset_symbol: 'USDT',
          duration_seconds: 120,
          payout_rate: '0.90',
          min_stake: '20',
          max_stake: '2000',
          status: 'active',
        },
        {
          id: 8,
          pair_id: 2,
          symbol: 'ETH-USDT',
          stake_asset: 2,
          stake_asset_symbol: 'USDT',
          duration_seconds: 60,
          payout_rate: '0.80',
          min_stake: '10',
          max_stake: '1000',
          status: 'disabled',
        },
        {
          id: 9,
          pair_id: 3,
          symbol: 'SOLUSDT',
          stake_asset: 2,
          stake_asset_symbol: 'USDT',
          duration_seconds: 60,
          payout_rate: '0.80',
          min_stake: '10',
          max_stake: '1000',
          status: 'active',
        },
      ],
    },
    {
      BTCUSDT: {
        symbol: 'BTCUSDT',
        last_price: '70000',
        open_24h: '69000',
        high_24h: '71000',
        low_24h: '68000',
        volume_24h: '12.5',
        price_change_percent_24h: '1.45',
        observed_at: 1_717_171_000_000,
      },
    },
  )
  assert.deepEqual(secondsTickers.map(ticker => ticker.symbol), ['BTC/USDT', 'SOL/USDT'])
  assert.equal(secondsTickers[0].icon, 'https://cdn.example.com/btc.png')
  assert.equal(secondsTickers[0].close, 70000)
  assert.equal(secondsTickers[0].high, 71000)
  assert.equal(secondsTickers[0].low, 68000)
  assert.equal(secondsTickers[0].volume, 12.5)
  assert.equal(secondsTickers[0].chg, 1.45)
  assert.equal(secondsTickers[1].close, 0)

  assert.deepEqual(mapPcSecondsOrderRequest({ symbol: 'BTC/USDT', coinSymbol: 'USDT', direction: 0, cycleId: 502, productId: 5, durationSeconds: 120, amount: 25 }, 'sec-1'), {
    product_id: 5,
    duration_seconds: 120,
    direction: 'up',
    stake_amount: '25',
    idempotency_key: 'sec-1',
  })

  assert.deepEqual(mapPcSecondsOrderRequest({ symbol: 'BTC/USDT', coinSymbol: 'USDT', direction: 0, cycleId: 4, amount: 25 }, 'sec-2'), {
    product_id: 4,
    direction: 'up',
    stake_amount: '25',
    idempotency_key: 'sec-2',
  })

  const orders = mapSecondsOrdersToPcOrders({
    orders: [
      {
        id: 31,
        user_id: 7,
        product_id: 4,
        pair_id: 1,
        symbol: 'BTC/USDT',
        stake_asset: 2,
        stake_asset_symbol: 'USDT',
        direction: 'down',
        stake_amount: '25',
        duration_seconds: 120,
        payout_rate: '0.85',
        entry_price: '70000',
        status: 'opened',
        result: 'win',
        idempotency_key: 'sec-1',
        created_at: 1_717_170_880_000,
        expires_at: 1_717_171_000_000,
      },
    ],
  })
  assert.equal(orders.data[0].direction, 'SELL')
  assert.equal(orders.data[0].betAmount, 25)
  assert.equal(orders.data[0].cycleLength, 120)
  assert.equal(orders.data[0].cycleRate, 0.85)
  assert.equal(orders.data[0].status, 'OPEN')
  assert.equal(orders.data[0].result, 'WIN')
  assert.equal(orders.data[0].createTime, 1_717_170_880_000)
  assert.equal(orders.data[0].endTime, 1_717_171_000_000)
})

test('PC seconds snapshot uses seconds products instead of all market list', () => {
  const secondApi = readFileSync(resolve(import.meta.dirname, '../src/api/second.ts'), 'utf8')

  assert.doesNotMatch(secondApi, /fetchSecondSnapshot\s*=\s*fetchMarketSnapshot/)
  assert.doesNotMatch(secondApi, /backendApiUrl\('\/markets'\)/)
  assert.match(secondApi, /\/seconds-contracts\/products/)
  assert.match(secondApi, /mapSecondsProductsToPcTickers/)
  assert.match(secondApi, /\/markets\/\$\{encodeURIComponent\(symbol\)\}\/ticker/)
})

test('PC product API modules use Rust backend endpoints instead of legacy product endpoints or mocks', () => {
  const apiDir = resolve(import.meta.dirname, '../src/api')
  const productModules = ['swap.ts', 'finance.ts', 'activity.ts', 'second.ts', 'option.ts', 'contract.ts']
  const source = productModules.map((file) => readFileSync(resolve(apiDir, file), 'utf8')).join('\n')

  assert.doesNotMatch(source, /\/uc\/ai-finance|\/uc\/activity|\/swap\/|\/second\/|\/option\//)
  assert.doesNotMatch(source, /mockOrders|Success \(Mock\)|setTimeout\s*\(/)
  assert.match(source, /\/convert\/pairs/)
  assert.match(source, /\/earn\/products/)
  assert.match(source, /\/new-coins/)
  assert.match(source, /\/seconds-contracts\/products/)
  assert.match(source, /\/margin\/products/)
})

test('PC swap page uses backend quote and user center shows convert orders', () => {
  const apiDir = resolve(import.meta.dirname, '../src/api')
  const srcDir = resolve(import.meta.dirname, '../src')
  const swapApi = readFileSync(resolve(apiDir, 'swap.ts'), 'utf8')
  const swapPage = readFileSync(resolve(srcDir, 'views/Swap.vue'), 'utf8')
  const transactionPage = readFileSync(resolve(srcDir, 'views/User/Transaction.vue'), 'utf8')
  const i18n = readFileSync(resolve(srcDir, 'i18n/index.ts'), 'utf8')

  assert.match(swapApi, /backendApiUrl\('\/convert\/pairs'\)/)
  assert.match(swapApi, /backendApiUrl\('\/convert\/quote'\)/)
  assert.match(swapApi, /backendApiUrl\('\/convert\/confirm'\)/)
  assert.match(swapApi, /backendApiUrl\('\/convert\/orders'\)/)
  assert.match(swapPage, /fetchSwapPairs/)
  assert.match(swapPage, /requestSwapQuote/)
  assert.match(swapPage, /confirmSwapQuote/)
  assert.doesNotMatch(swapPage, /fetchSwapOrders|recent_orders/)
  assert.match(transactionPage, /fetchSwapOrders/)
  assert.match(transactionPage, /nav\.transaction/)
  assert.match(transactionPage, /transaction\.all_types/)
  assert.match(transactionPage, /swap\.recent_orders/)
  assert.match(transactionPage, /transaction-tabs/)
  assert.match(transactionPage, /activeTab === 'transactions'/)
  assert.match(transactionPage, /activeTab === 'swapOrders'/)
  assert.doesNotMatch(transactionPage, />\s*Transaction History\s*</)
  assert.doesNotMatch(transactionPage, /swap\.pair/)
  assert.doesNotMatch(transactionPage, /order\.fromUnit\s*}}\/{{\s*order\.toUnit/)
  assert.match(i18n, /recent_orders: '闪兑记录'/)
  assert.match(i18n, /all_types: '全部类型'/)
  assert.match(swapPage, /assetSearch/)
  assert.match(swapPage, /PairLogo/)
  assert.doesNotMatch(swapPage, /<select/)
  assert.doesNotMatch(swapPage, /useMarketStore|marketStore|currentPrice/)
})

test('maps backend referral and public news payloads into PC user-center shapes', () => {
  const invites = mapMyInvitesToPcInviteRecords({
    users: [
      {
        user_id: 18,
        email: 'invitee@example.test',
        phone: null,
        status: 'active',
        direct_inviter_type: 'user',
        direct_inviter_id: 7,
        root_agent_id: null,
        depth: 1,
        path: '/7/18',
        created_at: 1_717_171_000_000,
      },
    ],
  })
  assert.equal(invites.code, 0)
  assert.equal(invites.data[0].invitee, 'i***@example.test')
  assert.equal(invites.data[0].status, 'Completed')
  assert.equal(invites.data[0].reward, '-')

  const news = mapPublicNewsItemsToPcNewsCards({
    news: [
      {
        id: 3,
        title: 'System Maintenance',
        category: 'system',
        status: 'published',
        country_code: null,
        default_locale: 'en',
        content_json: {
          version: 1,
          default_locale: 'en',
          items: [
            { locale: 'en', summary: 'Maintenance window', content: 'Full maintenance content' },
          ],
        },
        published_at: 1_717_171_000_000,
        created_at: 1_717_170_000_000,
        updated_at: 1_717_171_500_000,
      },
    ],
  })
  assert.equal(news[0].id, 3)
  assert.equal(news[0].category, 'system')
  assert.equal(news[0].summary, 'Maintenance window')
  assert.equal(news[0].content, 'Full maintenance content')
  assert.equal(news[0].source, 'MarketEx Team')
})

test('keeps backend public news categories aligned with admin configuration', () => {
  const news = mapPublicNewsItemsToPcNewsCards({
    news: ['general', 'market', 'product', 'system', 'promotion'].map((category, index) => ({
      id: index + 1,
      title: `${category} news`,
      category,
      status: 'published',
      country_code: 'GLOBAL',
      default_locale: 'en',
      content_json: {
        version: 1,
        default_locale: 'en',
        items: [{ locale: 'en', summary: `${category} summary`, content: `${category} body` }],
      },
      published_at: 1_717_171_000_000 + index,
      created_at: 1_717_170_000_000,
      updated_at: 1_717_171_500_000,
    })),
  })

  assert.deepEqual(news.map(item => item.category), ['general', 'market', 'product', 'system', 'promotion'])
})

test('selects public news locale families and renders backend rich text blocks for PC details', () => {
  const response = {
    news: [
      {
        id: 4,
        title: 'Fallback Title',
        category: 'market',
        status: 'published',
        country_code: 'CN',
        default_locale: 'zh-CN',
        content_json: {
          version: 1,
          default_locale: 'zh-CN',
          items: [
            { locale: 'en-US', summary: 'English summary', content: [{ type: 'p', children: [{ text: 'English content' }] }] },
            {
              locale: 'zh-CN',
              summary: [{ type: 'p', children: [{ text: '富文本摘要', bold: true }, { text: ' 与更多' }] }],
              title: '中文标题',
              content: [
                { type: 'h2', children: [{ text: '中文标题', bold: true }] },
                { type: 'p', children: [{ text: '<中文正文>', italic: true }] },
                { type: 'image', url: 'https://cdn.example.test/news.png', alt: '新闻配图' },
              ],
            },
          ],
        },
        published_at: 1_717_171_000_000,
        created_at: 1_717_170_000_000,
        updated_at: 1_717_171_500_000,
      },
    ],
  }

  assert.equal(mapPublicNewsItemsToPcNewsCards(response, 'en')[0].summary, 'English summary')
  const zhNews = mapPublicNewsItemsToPcNewsCards(response, 'zh')[0]
  assert.equal(zhNews.title, '中文标题')
  assert.equal(zhNews.summary, '富文本摘要 与更多')
  assert.equal(zhNews.content, '<h2><strong>中文标题</strong></h2><p><em>&lt;中文正文&gt;</em></p><figure><img src="https://cdn.example.test/news.png" alt="新闻配图" /></figure>')
  assert.equal(mapPublicNewsItemsToPcNewsCards(response, 'fr')[0].summary, '富文本摘要 与更多')
  assert.equal(mapPublicNewsItemsToPcNewsCards({
    news: [{ ...response.news[0], default_locale: undefined, content_json: { items: response.news[0].content_json.items } }],
  }, 'fr')[0].summary, 'English summary')
})

test('PC country locale wiring uses the new backend country and news contracts', () => {
  const apiDir = resolve(import.meta.dirname, '../src/api')
  const srcDir = resolve(import.meta.dirname, '../src')
  const sources = {
    auth: readFileSync(resolve(apiDir, 'auth.ts'), 'utf8'),
    countries: readFileSync(resolve(apiDir, 'countries.ts'), 'utf8'),
    register: readFileSync(resolve(srcDir, 'views/auth/Register.vue'), 'utf8'),
    setting: readFileSync(resolve(srcDir, 'stores/setting.ts'), 'utf8'),
    header: readFileSync(resolve(srcDir, 'components/layout/Header.vue'), 'utf8'),
    news: readFileSync(resolve(srcDir, 'views/News.vue'), 'utf8'),
    newsApi: readFileSync(resolve(apiDir, 'news.ts'), 'utf8'),
  }

  assert.match(sources.auth, /country_code:\s*data\.countryCode/)
  assert.match(sources.auth, /\/auth\/register\/config/)
  assert.match(sources.auth, /\/auth\/register\/email-code/)
  assert.match(sources.auth, /code:\s*data\.code/)
  assert.match(sources.auth, /invite_code:\s*data\.inviteCode/)
  assert.match(sources.countries, /backendApiUrl\('\/countries'\)/)
  assert.match(sources.register, /fetchPublicCountries/)
  assert.match(sources.register, /getRegisterConfig/)
  assert.match(sources.register, /inviteCodeRequired/)
  assert.match(sources.register, /countryCode:\s*form\.value\.countryCode/)
  assert.match(sources.register, /code:\s*form\.value\.code/)
  assert.match(sources.register, /inviteCode:\s*form\.value\.promotion/)
  assert.match(sources.register, /register_no_countries/)
  assert.match(sources.setting, /localeOverridden/)
  assert.match(sources.setting, /applyProfileLocale/)
  assert.match(sources.header, /availableLanguages/)
  assert.match(sources.header, /setManualLocale/)
  assert.match(sources.news, /countryCode:\s*userStore\.user\?\.countryCode/)
  assert.match(sources.news, /locale:\s*settingStore\.locale/)
  assert.match(sources.news, /category:\s*'general'/)
  assert.match(sources.news, /category:\s*'market'/)
  assert.match(sources.news, /category:\s*'product'/)
  assert.match(sources.news, /category:\s*'system'/)
  assert.match(sources.news, /category:\s*'promotion'/)
  assert.doesNotMatch(sources.news, /category:\s*'flash'/)
  assert.doesNotMatch(sources.newsApi, /pcCategoryToBackend/)
})

test('PC 2FA login security and withdrawal screens use the Rust security endpoints', () => {
  const apiDir = resolve(import.meta.dirname, '../src/api')
  const srcDir = resolve(import.meta.dirname, '../src')
  const sources = {
    auth: readFileSync(resolve(apiDir, 'auth.ts'), 'utf8'),
    user: readFileSync(resolve(apiDir, 'user.ts'), 'utf8'),
    wallet: readFileSync(resolve(apiDir, 'wallet.ts'), 'utf8'),
    login: readFileSync(resolve(srcDir, 'views/auth/Login.vue'), 'utf8'),
    security: readFileSync(resolve(srcDir, 'views/User/Security.vue'), 'utf8'),
    recharge: readFileSync(resolve(srcDir, 'views/User/Recharge.vue'), 'utf8'),
    withdraw: readFileSync(resolve(srcDir, 'views/User/Withdraw.vue'), 'utf8'),
  }

  assert.match(sources.auth, /\/auth\/login\/2fa/)
  assert.match(sources.auth, /\/auth\/login\/config/)
  assert.match(sources.auth, /usernameLoginEnabled/)
  assert.match(sources.auth, /\/auth\/login\/2fa\/reset-code/)
  assert.match(sources.auth, /\/auth\/login\/2fa\/reset/)
  assert.match(sources.login, /requires2fa/)
  assert.match(sources.login, /usernameLoginEnabled/)
  assert.match(sources.login, /email_or_username/)
  assert.match(sources.login, /submitLoginTwoFactor/)
  assert.match(sources.login, /resetLoginTwoFactor/)
  assert.match(sources.user, /\/user\/2fa/)
  assert.match(sources.user, /\/user\/username/)
  assert.match(sources.user, /\/user\/2fa\/setup/)
  assert.match(sources.user, /\/user\/2fa\/confirm/)
  assert.match(sources.user, /\/user\/2fa\/login/)
  assert.match(sources.user, /\/user\/2fa\/reset-code/)
  assert.match(sources.user, /\/user\/2fa\/reset/)
  assert.match(sources.security, /getTwoFactorStatus/)
  assert.match(sources.security, /confirmTwoFactor/)
  assert.match(sources.security, /updateLoginTwoFactor/)
  assert.match(sources.security, /updateUsername/)
  assert.match(sources.security, /showUsernameModal/)
  assert.match(sources.security, /from 'qrcode'/)
  assert.match(sources.security, /twoFactorQrCodeUrl/)
  assert.match(sources.security, /toDataURL\(setup\.otpauth_uri/)
  assert.doesNotMatch(sources.security, /{{\s*twoFactorSetup\.otpauth_uri\s*}}/)
  assert.doesNotMatch(sources.security, /security\.login_policy/)
  assert.doesNotMatch(sources.security, /login_2fa_mode\s*\|\|/)
  assert.match(sources.wallet, /\/wallet\/withdrawals/)
  assert.match(sources.wallet, /\/wallet\/deposit-assets/)
  assert.match(sources.wallet, /\/wallet\/deposit-networks/)
  assert.match(sources.wallet, /\/wallet\/withdraw-assets/)
  assert.match(sources.wallet, /\/wallet\/deposit-address/)
  assert.match(sources.wallet, /fetchWithdrawCoins/)
  assert.match(sources.wallet, /min_deposit_amount/)
  assert.match(sources.wallet, /deposit_fee/)
  assert.match(sources.wallet, /withdraw_fee/)
  assert.match(sources.wallet, /withdraw_fee_tiers/)
  assert.match(sources.withdraw, /calculateWithdrawFee/)
  assert.match(sources.withdraw, /fetchWithdrawCoins/)
  assert.match(sources.wallet, /\/wallet\/quick-recharge\/config/)
  assert.match(sources.wallet, /\/wallet\/quick-recharge\/orders/)
  assert.match(sources.wallet, /return_target/)
  assert.match(sources.wallet, /mapPcWithdrawalRequest/)
  assert.match(sources.wallet, /getNetworkInfo/)
  assert.match(sources.recharge, /getDepositAddress/)
  assert.match(sources.recharge, /selectedNetworkKey/)
  assert.match(sources.recharge, /createQuickRechargeOrder/)
  assert.match(sources.recharge, /activeRechargeTab/)
  assert.match(sources.recharge, /wallet\.normal_deposit/)
  assert.match(sources.recharge, /wallet\.quick_deposit/)
  assert.match(sources.recharge, /detectQuickRechargeReturnTarget/)
  assert.match(sources.recharge, /desktop_web/)
  assert.match(sources.recharge, /pc_app/)
  assert.match(sources.withdraw, /fundPassword/)
  assert.match(sources.withdraw, /totpCode/)
  assert.match(sources.withdraw, /getTwoFactorStatus/)
  assert.match(sources.withdraw, /getNetworkInfo/)
  assert.doesNotMatch(sources.withdraw, /getDepositAddress/)
})

test('PC residual user-center modules do not keep legacy endpoints or fake successes', () => {
  const apiDir = resolve(import.meta.dirname, '../src/api')
  const srcDir = resolve(import.meta.dirname, '../src')
  const files = [
    resolve(apiDir, 'wallet.ts'),
    resolve(apiDir, 'loan.ts'),
    resolve(apiDir, 'user.ts'),
    resolve(apiDir, 'news.ts'),
    resolve(srcDir, 'views/User/Invite.vue'),
    resolve(srcDir, 'views/News.vue'),
    resolve(srcDir, 'views/User/KYC.vue'),
    resolve(srcDir, 'views/User/Recharge.vue'),
    resolve(srcDir, 'views/User/Withdraw.vue'),
    resolve(srcDir, 'views/User/Security.vue'),
    resolve(srcDir, 'views/Loan.vue'),
    resolve(srcDir, 'views/User/LoanOrders.vue'),
    resolve(srcDir, 'router/index.ts'),
    resolve(srcDir, 'components/layout/Header.vue'),
    resolve(srcDir, 'views/User/UserLayout.vue'),
  ]
  const source = files.map((file) => readFileSync(file, 'utf8')).join('\n')

  assert.doesNotMatch(source, /\/uc\/|\/approve\/|installment/)
  assert.doesNotMatch(source, /mockRecords|Mock|mock|Math\.random\(|setTimeout\s*\(/)
  assert.doesNotMatch(source, /暂未开放资金密码重置接口/)
  assert.doesNotMatch(source, /当前后端暂未开放链上充值和提现接口/)
  assert.match(source, /\/referral\/my-code/)
  assert.match(source, /\/referral\/my-invites/)
  assert.match(source, /\/news/)
  assert.match(source, /\/user\/fund-password\/reset-code/)
  assert.match(source, /\/user\/fund-password\/reset/)
  assert.match(source, /\/wallet\/deposit-address/)
  assert.match(source, /\/wallet\/deposit-networks/)
  assert.match(source, /\/wallet\/quick-recharge\/orders/)
  assert.match(source, /return_target/)
  assert.match(source, /\/wallet\/withdrawals/)
})

test('PC home exposes direct news center entries', () => {
  const srcDir = resolve(import.meta.dirname, '../src')
  const home = readFileSync(resolve(srcDir, 'views/Home.vue'), 'utf8')
  const ticker = readFileSync(resolve(srcDir, 'components/home/NewsTicker.vue'), 'utf8')

  assert.match(home, /\$router\.push\('\/news'\)/)
  assert.match(home, /home\.news_cta/)
  assert.match(ticker, /to="\/news"/)
  assert.match(ticker, /home\.news_more/)
})

test('PC news detail uses a dedicated article route', () => {
  const srcDir = resolve(import.meta.dirname, '../src')
  const router = readFileSync(resolve(srcDir, 'router/index.ts'), 'utf8')
  const news = readFileSync(resolve(srcDir, 'views/News.vue'), 'utf8')
  const i18n = readFileSync(resolve(srcDir, 'i18n/index.ts'), 'utf8')

  assert.match(router, /path:\s*'news\/detail\/:id'/)
  assert.match(router, /name:\s*'NewsDetail'/)
  assert.match(news, /isDetailMode/)
  assert.match(news, /router\.push\(\{\s*name:\s*'NewsDetail'/s)
  assert.match(news, /fetchPublicNewsDetail\(id,\s*settingStore\.locale\)/)
  assert.match(news, /news\.back_to_news/)
  assert.match(news, /news\.related_news/)
  assert.match(news, /news-detail-shell/)
  assert.match(news, /news-detail-prose/)
  assert.match(news, /news\.article_info/)
  assert.match(news, /news\.latest_updates/)
  assert.match(news, /lg:sticky/)
  assert.match(news, /detailHotNews/)
  assert.doesNotMatch(news, /fixed inset-0 z-50/)
  assert.match(i18n, /back_to_news:\s*'返回资讯中心'/)
  assert.match(i18n, /related_news:\s*'相关推荐'/)
  assert.match(i18n, /article_info:\s*'文章信息'/)
  assert.match(i18n, /latest_updates:\s*'最新动态'/)
})

test('maps backend margin products, positions, and open request into PC contract shapes', () => {
  const products = mapMarginProductsToContractCoins({
    products: [
      {
        id: 6,
        pair_id: 1,
        symbol: 'ETH/USDT',
        logo_url: 'https://cdn.example.com/eth-usdt.png',
        margin_asset: 2,
        margin_asset_symbol: 'USDT',
        margin_mode: 'isolated',
        margin_modes: ['isolated', 'cross'],
        leverage_levels: ['1', '3', '5'],
        max_leverage: '5',
        min_margin: '20',
        max_margin: '5000',
        maintenance_margin_rate: '0.005',
        hourly_interest_rate: '0.0001',
        status: 'active',
      },
    ],
  })
  assert.equal(products.data[0].id, 6)
  assert.equal(products.data[0].symbol, 'ETH/USDT')
  assert.equal(products.data[0].logoUrl, 'https://cdn.example.com/eth-usdt.png')
  assert.deepEqual(products.data[0].leverage, [1, 3, 5])
  assert.deepEqual(products.data[0].marginModes, ['isolated', 'cross'])

  assert.deepEqual(mapPcMarginOpenRequest({ contractCoinId: 6, direction: 1, type: 0, leverage: 3, marginMode: 'cross', volume: 100 }, 'margin-1'), {
    product_id: 6,
    direction: 'short',
    margin_mode: 'cross',
    margin_amount: '100',
    leverage: '3',
    idempotency_key: 'margin-1',
  })

  const response = {
    positions: [
      {
        id: 9,
        user_id: 7,
        product_id: 6,
        pair_id: 1,
        symbol: 'ETH/USDT',
        margin_asset: 2,
        margin_asset_symbol: 'USDT',
        margin_mode: 'isolated',
        direction: 'long',
        margin_amount: '100',
        leverage: '3',
        notional_amount: '300',
        borrowed_amount: '200',
        interest_amount: '1',
        entry_price: '2500',
        exit_price: null,
        realized_pnl: null,
        closed_at: null,
        status: 'opened',
        idempotency_key: 'margin-1',
      },
    ],
  }
  assert.equal(mapMarginPositionsToContractOrders(response).data[0].direction, 0)
  assert.equal(mapMarginPositionsToContractOrders(response).data[0].amount, 300)
  assert.equal(mapMarginPositionsToContractOrders(response).data[0].status, 0)
  assert.equal(mapMarginPositionsToContractWallets(response).data[0].usdtBuyPosition, 300)
  assert.equal(mapMarginPositionsToContractWallets(response).data[0].usdtBuyPrincipalAmount, 100)
})
