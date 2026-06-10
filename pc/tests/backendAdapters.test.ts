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
  mapConvertPairsToPcCoins,
  mapEarnProductsToPcFinanceList,
  mapEarnSubscriptionsToPcFinanceCount,
  mapEarnSubscriptionsToPcFinancePage,
  mapEarnSubscriptionsToPcFinanceStatistic,
  mapMarginPositionsToContractOrders,
  mapMarginPositionsToContractWallets,
  mapMarginProductsToContractCoins,
  mapMyInvitesToPcInviteRecords,
  mapNewCoinProjectsToPcActivityPage,
  mapPublicNewsItemsToPcNewsCards,
  mapPcMarginOpenRequest,
  mapPcNewCoinSubscriptionRequest,
  mapPcSecondsOrderRequest,
  mapPcSpotOrderRequest,
  mapSecondsOrdersToPcOrders,
  mapSecondsProductsToPcCycles,
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
        volume_24h: '245.5',
        observed_at: 1_717_171_000_000,
      },
    },
  )

  assert.equal(tickers[0].symbol, 'BTC/USDT')
  assert.equal(tickers[0].close, 70001.12)
  assert.equal(tickers[0].volume, 245.5)
  assert.equal(tickers[0].time, 1_717_171_000_000)
  assert.equal(tickers[0].zone, 0)
  const updated = mapMarketTickerToPcTicker(tickers[0], {
    symbol: 'BTCUSDT',
    last_price: '70002',
    volume_24h: '250',
    observed_at: 1_717_171_001_000,
  })
  assert.equal(updated.close, 70002)
  assert.equal(updated.high, 70002)
  assert.equal(updated.low, 70001.12)
  assert.equal(updated.volume, 250)
  assert.equal(updated.turnover, 70002 * 250)
  assert.equal(updated.time, 1_717_171_001_000)
  assert.ok(updated.chg > 0)
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

test('normalizes backend profile into the current PC security shape', () => {
  const response = normalizeProfileForSecurity({
    id: 7,
    email: 'user@example.com',
    phone: null,
    status: 'active',
    kyc_level: 1,
    email_verified_at: 1_717_171_717_000,
    fund_password_set: true,
    created_at: 1_717_171_000_000,
  })

  assert.equal(response.code, 0)
  assert.equal(response.data.id, 7)
  assert.equal(response.data.email, 'user@example.com')
  assert.equal(response.data.username, 'user@example.com')
  assert.equal(response.data.emailVerified, 1)
  assert.equal(response.data.fundsVerified, 1)
  assert.equal(response.data.transactionStatus, 1)
})

test('maps backend wallet accounts into existing PC wallet rows', () => {
  const response = mapWalletAccountsToMemberWallets({
    accounts: [
      {
        user_id: 7,
        asset_id: 2,
        symbol: 'USDT',
        available: '100.5',
        frozen: '2',
        locked: '3',
      },
    ],
  })

  assert.equal(response.code, 0)
  assert.equal(response.data[0].memberId, 7)
  assert.equal(response.data[0].coin.coinGroup, 'USDT')
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
          status: 'open',
        },
      ],
    },
    { pageNo: 0, pageSize: 10 },
  )

  assert.equal(page.code, 0)
  assert.equal(page.data.content[0].orderId, '91')
  assert.equal(page.data.content[0].symbol, 'BTC/USDT')
  assert.equal(page.data.content[0].direction, 'BUY')
  assert.equal(page.data.content[0].type, 'LIMIT_PRICE')
  assert.equal(page.data.content[0].price, 70000)
  assert.equal(page.data.content[0].amount, 0.2)
  assert.equal(page.data.content[0].filledAmount, 0.05)
  assert.equal(page.data.content[0].status, 'TRADING')
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
    amount: 5000,
    price: 2500,
  }, 'spot-request-3'), {
    pair_id: 'ETH-USDT',
    side: 'buy',
    order_type: 'market',
    quantity: '2',
    reference_price: '2500',
    idempotency_key: 'spot-request-3',
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

test('maps backend wallet ledger into the current transaction history page shape', () => {
  const response = normalizeWalletLedgerPage(
    {
      entries: [
        {
          id: 11,
          user_id: 7,
          asset_id: 2,
          symbol: 'USDT',
          change_type: 'credit',
          amount: '10.25',
          balance_type: 'available',
          balance_after: '110.25',
          available_after: '110.25',
          frozen_after: '0',
          locked_after: '0',
          ref_type: 'admin_recharge',
          ref_id: 'r1',
          created_at: 1_717_171_000_000,
        },
      ],
    },
    { pageNo: 1, pageSize: 10 },
  )

  assert.equal(response.code, 0)
  assert.equal(response.data.content[0].id, 11)
  assert.equal(response.data.content[0].memberId, 7)
  assert.equal(response.data.content[0].symbol, 'USDT')
  assert.equal(response.data.content[0].amount, 10.25)
  assert.equal(response.data.content[0].status, 1)
  assert.equal(response.data.page.totalPages, 1)
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
        min_amount: '0.01',
        max_amount: '10',
        enabled: true,
      },
    ],
  })

  assert.equal(rows.code, 0)
  assert.deepEqual(rows.data, [
    { id: 8, fromUnit: 'ETH', toUnit: 'USDT', minAmount: 0.01, maxAmount: 10, enabled: true },
  ])
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
        user_id: 7,
        product_id: 3,
        asset_id: 2,
        asset_symbol: 'USDT',
        amount: '500',
        apr_rate: '0.12',
        term_days: 30,
        status: 'subscribed',
        idempotency_key: 'earn-1',
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
  assert.equal(page.data.content[0].coinSymbol, 'USDT')
  assert.equal(page.data.content[0].status, 0)
  assert.equal(page.data.content[0].num, 500)
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
  assert.deepEqual(cycles.data, [{ id: 4, cycleLength: 60, cycleRate: 0.85, minAmount: 10, maxAmount: 1000 }])

  assert.deepEqual(mapPcSecondsOrderRequest({ symbol: 'BTC/USDT', coinSymbol: 'USDT', direction: 0, cycleId: 4, amount: 25 }, 'sec-1'), {
    product_id: 4,
    direction: 'up',
    stake_amount: '25',
    idempotency_key: 'sec-1',
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
        payout_rate: '0.85',
        entry_price: '70000',
        status: 'opened',
        result: 'win',
        idempotency_key: 'sec-1',
        expires_at: 1_717_171_000_000,
      },
    ],
  })
  assert.equal(orders.data[0].direction, 'SELL')
  assert.equal(orders.data[0].betAmount, 25)
  assert.equal(orders.data[0].cycleRate, 0.85)
  assert.equal(orders.data[0].status, 'OPEN')
  assert.equal(orders.data[0].result, 'WIN')
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
  assert.equal(news[0].category, 'announcement')
  assert.equal(news[0].summary, 'Maintenance window')
  assert.equal(news[0].source, 'MarketEx Team')
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
    resolve(srcDir, 'views/OTC.vue'),
    resolve(srcDir, 'views/Loan.vue'),
    resolve(srcDir, 'views/User/LoanOrders.vue'),
    resolve(srcDir, 'router/index.ts'),
    resolve(srcDir, 'components/layout/Header.vue'),
    resolve(srcDir, 'views/User/UserLayout.vue'),
  ]
  const source = files.map((file) => readFileSync(file, 'utf8')).join('\n')

  assert.doesNotMatch(source, /\/uc\/|\/approve\/|installment/)
  assert.doesNotMatch(source, /mockRecords|Mock|mock|Math\.random\(|setTimeout\s*\(/)
  assert.match(source, /\/referral\/my-code/)
  assert.match(source, /\/referral\/my-invites/)
  assert.match(source, /\/news/)
})

test('maps backend margin products, positions, and open request into PC contract shapes', () => {
  const products = mapMarginProductsToContractCoins({
    products: [
      {
        id: 6,
        pair_id: 1,
        symbol: 'ETH/USDT',
        margin_asset: 2,
        margin_asset_symbol: 'USDT',
        margin_mode: 'isolated',
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
  assert.deepEqual(products.data[0].leverage, [1, 3, 5])

  assert.deepEqual(mapPcMarginOpenRequest({ contractCoinId: 6, direction: 1, type: 0, leverage: 3, volume: 100 }, 'margin-1'), {
    product_id: 6,
    direction: 'short',
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
