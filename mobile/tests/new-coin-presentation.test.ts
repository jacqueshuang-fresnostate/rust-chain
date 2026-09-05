import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import type {
  NewCoinDistribution,
  NewCoinProject,
  NewCoinPurchase,
  NewCoinSubscription,
  NewCoinUnlock,
} from '../src/core/newCoinModel.ts'
import {
  buildNewCoinOpportunities,
  buildUnifiedNewCoinRecords,
  filterNewCoinProjects,
  filterUnifiedNewCoinRecords,
  newCoinUnlockTypeTranslationKey,
  newCoinProjectProgress,
} from '../src/core/newCoinPresentation.ts'
import type { MarketTicker } from '../src/core/types.ts'

function project(overrides: Partial<NewCoinProject> = {}): NewCoinProject {
  return {
    id: 1,
    assetId: 11,
    symbol: 'ABC',
    lifecycleStatus: 'subscription',
    totalSupplyText: normalizeDecimalText('100'),
    issuePriceText: normalizeDecimalText('0.5'),
    quoteAssetId: 21,
    quoteAssetSymbol: 'USDT',
    reservedSupplyText: normalizeDecimalText('25'),
    allocatedSupplyText: normalizeDecimalText('5'),
    remainingSupplyText: normalizeDecimalText('75'),
    unlockType: 'fixed_time',
    unlockFeeEnabled: false,
    postListingPurchaseEnabled: false,
    status: 'active',
    ...overrides,
  }
}

function ticker(overrides: Partial<MarketTicker> = {}): MarketTicker {
  return {
    id: 41,
    symbol: 'ABC/USDT',
    base: 'ABC',
    quote: 'USDT',
    lastPrice: 1,
    lastPriceText: normalizeDecimalText('1'),
    openPrice: 0.9,
    highPrice: 1.1,
    lowPrice: 0.8,
    volume: 10,
    changePercent: 2,
    ...overrides,
  }
}

test('project filters match backend lifecycle values exactly', () => {
  const projects = [
    project({ id: 1, lifecycleStatus: 'preheat' }),
    project({ id: 2, lifecycleStatus: 'subscription' }),
    project({ id: 3, lifecycleStatus: 'distribution' }),
    project({ id: 4, lifecycleStatus: 'listed' }),
    project({ id: 5, lifecycleStatus: 'closed' }),
  ]

  assert.deepEqual(filterNewCoinProjects(projects, 'preheat').map(({ id }) => id), [1])
  assert.deepEqual(filterNewCoinProjects(projects, 'subscription').map(({ id }) => id), [2])
  assert.deepEqual(filterNewCoinProjects(projects, 'distribution').map(({ id }) => id), [3])
  assert.deepEqual(filterNewCoinProjects(projects, 'listed').map(({ id }) => id), [4])
  assert.equal(filterNewCoinProjects(projects, 'all').length, 5)
})

test('project progress uses exact decimal division and caps geometry at one', () => {
  assert.equal(newCoinProjectProgress(project()).ratio, '0.25')
  assert.equal(newCoinProjectProgress(project()).percentage, 25)
  assert.equal(newCoinProjectProgress(project({ reservedSupplyText: normalizeDecimalText('150') })).ratio, '1')
  assert.equal(newCoinProjectProgress(project({ totalSupplyText: normalizeDecimalText('0') })).percentage, 0)
})

test('trading opportunities require the configured pair id and never infer by symbol or direct-purchase flag', () => {
  const now = new Date(2026, 8, 5, 12).getTime()
  const projects = [
    project({ id: 1, lifecycleStatus: 'listed', postListingPurchaseEnabled: true, postListingPairId: 41, listedAt: now }),
    project({ id: 2, symbol: 'XYZ', lifecycleStatus: 'listed', postListingPurchaseEnabled: true, postListingPairId: 42, listedAt: now }),
    project({ id: 3, lifecycleStatus: 'listed', postListingPurchaseEnabled: false, postListingPairId: 43, listedAt: now - 86_400_000 }),
  ]
  const tickers = [
    ticker({ id: 41, changePercent: 2 }),
    ticker({ id: 43, changePercent: 3 }),
    ticker({ id: 99, symbol: 'XYZ/USDT', base: 'XYZ', changePercent: 20 }),
  ]

  assert.deepEqual(buildNewCoinOpportunities(projects, tickers, 'all', now).map(({ project: item }) => item.id), [1, 3])
  assert.equal(buildNewCoinOpportunities(projects, tickers, 'listedToday', now).length, 1)
  assert.deepEqual(buildNewCoinOpportunities(projects, tickers, 'hotGains', now).map(({ project: item }) => item.id), [3, 1])
})

test('supported backend unlock enums map symmetrically and unknown values stay available as fallbacks', () => {
  assert.equal(newCoinUnlockTypeTranslationKey('immediate_on_listing'), 'newCoin.immediateUnlock')
  assert.equal(newCoinUnlockTypeTranslationKey('fixed_time'), 'newCoin.fixedUnlock')
  assert.equal(newCoinUnlockTypeTranslationKey('relative_period'), 'newCoin.relativeUnlock')
  assert.equal(newCoinUnlockTypeTranslationKey('future_policy'), undefined)
})

test('four record sources merge chronologically and filter without changing source identity', () => {
  const subscription: NewCoinSubscription = {
    id: 1, projectId: 1, quoteAssetId: 21,
    issuePriceText: normalizeDecimalText('1'), quoteAmountText: normalizeDecimalText('10'),
    requestedQuantityText: normalizeDecimalText('10'), allocatedQuantityText: normalizeDecimalText('0'),
    status: 'pending', idempotencyKey: 'sub-1', createdAt: 100,
  }
  const distribution: NewCoinDistribution = {
    id: 2, projectId: 1, assetId: 11, quantityText: normalizeDecimalText('5'),
    status: 'distributed', idempotencyKey: 'dist-2', createdAt: 400,
  }
  const purchase: NewCoinPurchase = {
    id: 3, projectId: 1, pairId: 41, baseAssetId: 11, quoteAssetId: 21,
    priceText: normalizeDecimalText('1'), quantityText: normalizeDecimalText('2'), quoteAmountText: normalizeDecimalText('2'),
    status: 'locked', idempotencyKey: 'buy-3', createdAt: 200,
  }
  const unlock: NewCoinUnlock = {
    id: 4, assetId: 11, lockPositionId: 51, unlockQuantityText: normalizeDecimalText('2'),
    unlockFeeEnabled: true, unlockFeeAssetId: 21, unlockFeeAmountText: normalizeDecimalText('0.1'),
    feePaidStatus: 'pending', status: 'pending', idempotencyKey: 'unlock-4', createdAt: 300,
  }
  const records = buildUnifiedNewCoinRecords({
    projects: [project()], subscriptions: [subscription], distributions: [distribution], purchases: [purchase], unlocks: [unlock],
  })

  assert.deepEqual(records.map(({ key }) => key), ['distribution-2', 'unlock-4', 'purchase-3', 'subscription-1'])
  assert.equal(records[1]?.statusBucket, 'pendingSettlement')
  assert.deepEqual(filterUnifiedNewCoinRecords(records, 'all', 'completed').map(({ key }) => key), ['distribution-2'])
  assert.deepEqual(filterUnifiedNewCoinRecords(records, 'purchases', 'pendingSettlement').map(({ key }) => key), ['purchase-3'])
  assert.equal(records[0]?.recordNo, 'dist-2')
})
