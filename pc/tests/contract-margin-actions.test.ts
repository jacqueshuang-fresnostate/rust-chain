import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  findClosablePositionByAction,
  isMarginSettingRequestCurrent,
  resolveMarginPositionSymbol,
  resolveSelectedMarginMode,
  summarizeMarginBatchAction,
} from '../src/domain/marginActions.ts'
import {
  mapMarginProductsToContractCoins,
  mapMarginPositionsToContractOrders,
  mapMarginWalletsToContractWallets,
  mapPcMarginOpenRequest,
} from '../src/api/backendAdapters.ts'

const pcRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

test('close-long and close-short resolve the exact position id in a hedge fixture', () => {
  const positions = [
    { id: 901, symbol: 'BTC/USDT', usdtBuyPosition: 2, usdtBuyPrice: 100, usdtSellPosition: 0, usdtSellPrice: 0 },
    { id: 902, symbol: 'BTC-USDT', usdtBuyPosition: 0, usdtBuyPrice: 0, usdtSellPosition: 3, usdtSellPrice: 100 },
  ]

  assert.equal(findClosablePositionByAction(positions, 'BTC_USDT', 'close_long')?.id, 901)
  assert.equal(findClosablePositionByAction(positions, 'BTC_USDT', 'close_short')?.id, 902)
})

test('close selection ignores pending, malformed, and empty-symbol rows', () => {
  const positions = [
    { id: 800, symbol: 'BTC/USDT', usdtBuyPosition: 5, usdtBuyPrice: 0, usdtSellPosition: 0, usdtSellPrice: 0 },
    { id: 801, symbol: 'BTC/USDT', usdtBuyPosition: -1, usdtBuyPrice: 100, usdtSellPosition: 0, usdtSellPrice: 0 },
    { id: 802, symbol: 'BTC/USDT', usdtBuyPosition: 2, usdtBuyPrice: 100, usdtSellPosition: 0, usdtSellPrice: 0 },
  ]

  assert.equal(findClosablePositionByAction(positions, 'BTC-USDT', 'close_long')?.id, 802)
  assert.equal(findClosablePositionByAction(positions, '', 'close_long'), null)
})

test('selected margin mode stays inside capability-product intersection and honors setting', () => {
  assert.equal(resolveSelectedMarginMode(['cross', 'isolated'], 'cross'), 'cross')
  assert.equal(resolveSelectedMarginMode(['isolated'], 'cross'), null)
  assert.equal(resolveSelectedMarginMode(['cross'], 'invalid'), null)
  assert.equal(resolveSelectedMarginMode([], 'isolated'), null)
})

test('stale setting responses cannot overwrite another active product', () => {
  assert.equal(isMarginSettingRequestCurrent(31, 31, 4, 4), true)
  assert.equal(isMarginSettingRequestCurrent(31, 32, 4, 4), false)
  assert.equal(isMarginSettingRequestCurrent(31, 31, 4, 5), false)
})

test('product capability intersection supplies the selected mode and market-only open request', () => {
  const mapped = mapMarginProductsToContractCoins({
    capabilities: { margin_modes: ['cross'], order_types: ['market', 'limit'] },
    products: [{
      id: 31,
      pair_id: 88,
      symbol: 'BTC-USDT',
      margin_asset: 7,
      margin_asset_symbol: 'USDT',
      margin_mode: 'cross',
      margin_modes: ['isolated', 'cross'],
      leverage_levels: ['2', '5'],
      max_leverage: '5',
      min_margin: '10',
      maintenance_margin_rate: '0.05',
      hourly_interest_rate: '0.001',
      status: 'active',
    }],
  }).data[0]

  assert.deepEqual(mapped.marginModes, ['cross'])
  assert.equal(mapped.marginMode, 'cross')
  const request = mapPcMarginOpenRequest({
    contractCoinId: 31,
    direction: 0,
    leverage: 5,
    marginMode: resolveSelectedMarginMode(mapped.marginModes, 'cross') ?? undefined,
    volume: 10,
  }, 'mode-contract')
  assert.deepEqual(request, {
    product_id: 31,
    direction: 'long',
    order_type: 'market',
    margin_mode: 'cross',
    margin_amount: '10',
    leverage: '5',
    idempotency_key: 'mode-contract',
  })
})

test('position id remains paired with its product symbol before close action selection', () => {
  const products = [{ id: 31, symbol: 'BTC/USDT' }]
  const position = {
    id: 901,
    symbol: resolveMarginPositionSymbol('88', 31, products),
    usdtBuyPosition: 2,
    usdtBuyPrice: 100,
    usdtSellPosition: 0,
    usdtSellPrice: 0,
  }
  assert.equal(findClosablePositionByAction([position], 'BTC-USDT', 'close_long')?.id, 901)
})

test('margin order DTO preserves market/limit type, limit price, fill state, and pending status', () => {
  const rows = mapMarginPositionsToContractOrders({
    positions: [
      {
        id: 701,
        user_id: 9,
        product_id: 31,
        pair_id: 88,
        symbol: 'BTC-USDT',
        margin_asset: 7,
        margin_mode: 'isolated',
        direction: 'long',
        order_type: 'limit',
        limit_price: '88',
        margin_amount: '10',
        leverage: '5',
        notional_amount: '50',
        borrowed_amount: '40',
        interest_amount: '0',
        entry_price: null,
        status: 'pending',
        idempotency_key: 'pending-limit',
      },
      {
        id: 702,
        user_id: 9,
        product_id: 31,
        pair_id: 88,
        symbol: 'BTC-USDT',
        margin_asset: 7,
        margin_mode: 'cross',
        direction: 'short',
        order_type: 'market',
        margin_amount: '10',
        leverage: '5',
        notional_amount: '50',
        borrowed_amount: '40',
        interest_amount: '0',
        entry_price: '100',
        status: 'opened',
        idempotency_key: 'filled-market',
      },
    ],
  }).data as Array<{ orderId: string; price: number; type: number; tradedAmount: number; status: number }>

  assert.deepEqual(
    [rows[0].orderId, rows[0].price, rows[0].type, rows[0].tradedAmount, rows[0].status],
    ['701', 88, 0, 0, 0],
  )
  assert.deepEqual(
    [rows[1].orderId, rows[1].price, rows[1].type, rows[1].tradedAmount, rows[1].status],
    ['702', 100, 1, 50, 0],
  )
})

test('batch failures are never summarized as a pure success', () => {
  assert.equal(summarizeMarginBatchAction({ succeeded: ['1'], failures: [] }), 'success')
  assert.equal(
    summarizeMarginBatchAction({
      succeeded: ['1'],
      failures: [{ id: '2', code: 'CONFLICT', message: 'still open' }],
    }),
    'partial_failure',
  )
  assert.equal(
    summarizeMarginBatchAction({
      succeeded: [],
      failures: [{ id: '2', code: 'CONFLICT', message: 'still open' }],
    }),
    'failure',
  )
})

test('missing transfer risk authority never falls back to the raw available balance', () => {
  const mapped = mapMarginWalletsToContractWallets({
    wallets: [{
      asset_id: 7,
      asset_symbol: 'USDT',
      available: '100',
      frozen: '0',
      locked: '0',
    }],
    positions: [],
  }).data[0] as { maxTransferableToSpot: number }

  assert.equal(mapped.maxTransferableToSpot, 0)

  const malformed = mapMarginWalletsToContractWallets({
    wallets: [{
      asset_id: 7,
      asset_symbol: 'USDT',
      available: '100',
      frozen: '0',
      locked: '0',
      max_transferable_to_spot: '-1',
    }],
    positions: [],
  }).data[0] as { maxTransferableToSpot: number }
  assert.equal(malformed.maxTransferableToSpot, 0)
})

test('wallet adapter preserves the authoritative cross-transfer risk DTO', () => {
  const mapped = mapMarginWalletsToContractWallets({
    wallets: [{
      asset_id: 7,
      asset_symbol: 'USDT',
      margin_transfer_enabled: true,
      available: '100',
      frozen: '0',
      locked: '0',
      max_transferable_to_spot: '30',
      transfer_to_spot_block_reason: null,
      cross_account_version: 8,
      transfer_risk_equity: '50',
      transfer_risk_maintenance_margin: '20',
      transfer_risk_observed_at: 1_900_000_000_000,
    }],
    positions: [],
  }).data[0] as Record<string, unknown>

  assert.equal(mapped.marginTransferEnabled, true)
  assert.equal(mapped.maxTransferableToSpot, 30)
  assert.equal(mapped.crossAccountVersion, 8)
  assert.equal(mapped.transferRiskEquity, 50)
  assert.equal(mapped.transferRiskMaintenanceMargin, 20)
  assert.equal(mapped.transferRiskObservedAt, 1_900_000_000_000)
})

test('PC close contract exposes only a direct market full-close action', () => {
  const api = readFileSync(resolve(pcRoot, 'src/api/contract.ts'), 'utf8')
  const form = readFileSync(resolve(pcRoot, 'src/components/trade/ContractOrderForm.vue'), 'utf8')
  const orders = readFileSync(resolve(pcRoot, 'src/components/trade/ContractOrders.vue'), 'utf8')
  const closeParams = api.match(/export interface ClosePositionParams \{([\s\S]*?)\n\}/)?.[1] ?? ''

  assert.match(closeParams, /positionId: string/)
  assert.doesNotMatch(closeParams, /direction|type|triggerPrice|entrustPrice|volume/)
  assert.match(api, /params\.positionId/)
  assert.doesNotMatch(api, /resolveOpenPositionId/)
  assert.match(form, /findClosablePositionByAction\(contractStore\.wallets, props\.symbol, action\)/)
  assert.match(form, /positionId: String\(target\.id\)/)
  assert.match(orders, /positionId: pos\.orderId/)
  assert.match(orders, /trade\.market_full_close/)
  assert.match(orders, /batchFailures\.value = result\.failures/)
  assert.match(orders, /data-testid="margin-batch-failures"/)
  assert.match(orders, /failure\.id/)
  assert.match(orders, /isEmpty && \(activeTab !== 'positions' \|\| batchFailures\.length === 0\)/)
  assert.match(form, /isMarginSettingRequestCurrent/)
  assert.doesNotMatch(orders, /closeOrderType|closePrice|closeVolume|setClosePercent/)
})
