import assert from 'node:assert/strict'
import test from 'node:test'
import {
  NewCoinContractError,
  mapNewCoinDistribution,
  mapNewCoinProject,
  mapNewCoinPurchase,
  mapNewCoinSubscription,
  mapNewCoinUnlock,
} from '../src/core/newCoinModel.ts'

function projectFixture(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: 11,
    asset_id: 21,
    symbol: 'abc',
    name: 'ABC Network',
    logo_url: ' https://assets.example.test/abc.png ',
    lifecycle_status: 'subscription',
    total_supply: '9007199254740993.000000000000000001',
    issue_price: '0.000000000000000001',
    quote_asset_id: 31,
    quote_asset_symbol: 'usdt',
    quote_asset_logo_url: 'https://assets.example.test/usdt.png',
    reserved_supply: '10.25',
    allocated_supply: '3.125',
    remaining_supply: '9007199254740982.75',
    listed_at: 1_720_000_000_000,
    unlock_type: 'fixed_time',
    fixed_unlock_at: null,
    relative_unlock_seconds: 86400,
    unlock_fee_enabled: true,
    unlock_fee_rate: '0.04',
    unlock_fee_basis: 'market_value',
    unlock_fee_asset: 31,
    post_listing_purchase_enabled: true,
    post_listing_pair_id: 41,
    status: 'active',
    ...overrides,
  }
}

test('new-coin project mapping preserves exact decimals and authoritative asset metadata', () => {
  const project = mapNewCoinProject(projectFixture())

  assert.equal(project.symbol, 'ABC')
  assert.equal(project.name, 'ABC Network')
  assert.equal(project.logoUrl, 'https://assets.example.test/abc.png')
  assert.equal(project.quoteAssetId, 31)
  assert.equal(project.quoteAssetSymbol, 'USDT')
  assert.equal(project.quoteAssetLogoUrl, 'https://assets.example.test/usdt.png')
  assert.equal(project.totalSupplyText, '9007199254740993.000000000000000001')
  assert.equal(project.issuePriceText, '0.000000000000000001')
  assert.equal(project.reservedSupplyText, '10.25')
  assert.equal(project.allocatedSupplyText, '3.125')
  assert.equal(project.remainingSupplyText, '9007199254740982.75')
  assert.equal(project.unlockFeeRateText, '0.04')
})

test('new-coin project mapping normalizes nullable blank metadata without inventing values', () => {
  const project = mapNewCoinProject(projectFixture({
    name: null,
    logo_url: '   ',
    quote_asset_id: null,
    quote_asset_symbol: null,
    quote_asset_logo_url: '',
    unlock_fee_rate: null,
    unlock_fee_basis: null,
    unlock_fee_asset: null,
    post_listing_pair_id: null,
  }))

  assert.equal(project.name, undefined)
  assert.equal(project.logoUrl, undefined)
  assert.equal(project.quoteAssetId, undefined)
  assert.equal(project.quoteAssetSymbol, undefined)
  assert.equal(project.quoteAssetLogoUrl, undefined)
  assert.equal(project.postListingPairId, undefined)
})

test('new-coin mapping rejects malformed logo, decimal, and boolean contracts', () => {
  assert.throws(() => mapNewCoinProject(projectFixture({ logo_url: 7 })), NewCoinContractError)
  assert.throws(() => mapNewCoinProject(projectFixture({ issue_price: 0.5 })), NewCoinContractError)
  assert.throws(() => mapNewCoinProject(projectFixture({ unlock_fee_enabled: 'false' })), NewCoinContractError)
  assert.throws(() => mapNewCoinProject(projectFixture({ id: 0 })), NewCoinContractError)
})

test('new-coin record mappings retain exact amounts, identifiers, and timestamps', () => {
  const subscription = mapNewCoinSubscription({
    id: 1,
    project_id: 11,
    quote_asset: 31,
    issue_price: '0.125',
    quote_amount: '9007199254740993.1',
    requested_quantity: '72.0000000000000008',
    allocated_quantity: '7.2',
    status: 'allocated',
    idempotency_key: 'subscription-1',
    created_at: 1_720_000_000,
  })
  const purchase = mapNewCoinPurchase({
    id: 2,
    project_id: 11,
    pair_id: 41,
    base_asset: 21,
    quote_asset: 31,
    price: '0.2',
    quantity: '3.3',
    quote_amount: '0.66',
    lock_position_id: null,
    status: 'locked',
    idempotency_key: 'purchase-2',
    created_at: 1_720_000_001_000,
  })
  const distribution = mapNewCoinDistribution({
    id: 4,
    project_id: 11,
    subscription_id: 1,
    asset_id: 21,
    quantity: '9007199254740993.000000000000000001',
    lock_position_id: 51,
    status: 'distributed',
    idempotency_key: 'distribution-4',
    created_at: 1_720_000_001,
  })
  const unlock = mapNewCoinUnlock({
    id: 3,
    asset_id: 21,
    lock_position_id: 51,
    unlock_quantity: '3.3',
    unlock_price: null,
    unlock_fee_enabled: true,
    unlock_fee_rate: '0.04',
    unlock_fee_basis: 'market_value',
    unlock_fee_asset: 31,
    unlock_fee_amount: '0.0264',
    fee_paid_status: 'pending',
    status: 'pending',
    idempotency_key: 'unlock-3',
    created_at: 1_720_000_002_000,
  })

  assert.equal(subscription.quoteAmountText, '9007199254740993.1')
  assert.equal(subscription.createdAt, 1_720_000_000_000)
  assert.equal(purchase.quoteAmountText, '0.66')
  assert.equal(purchase.lockPositionId, undefined)
  assert.equal(distribution.quantityText, '9007199254740993.000000000000000001')
  assert.equal(distribution.lockPositionId, 51)
  assert.equal(distribution.createdAt, 1_720_000_001_000)
  assert.equal(unlock.unlockFeeAmountText, '0.0264')
  assert.equal(unlock.unlockPriceText, undefined)
})

test('new-coin record mapping rejects legacy numeric financial fields', () => {
  assert.throws(() => mapNewCoinSubscription({
    id: 1,
    project_id: 11,
    quote_asset: 31,
    issue_price: '0.125',
    quote_amount: 10,
    requested_quantity: '80',
    allocated_quantity: '0',
    status: 'pending',
    idempotency_key: 'subscription-1',
    created_at: 1_720_000_000_000,
  }), NewCoinContractError)
})

test('manual subscription snapshots preserve exact frozen, paid and refunded decimals', () => {
  const raw = {
    id: 9, project_id: 11, quote_asset: 31, issue_price: '2.5', quote_amount: '25',
    requested_quantity: '10', allocated_quantity: '4', status: 'partial_allocated',
    idempotency_key: 'manual-9', created_at: 1720000000000,
    settlement_mode: 'manual_distribution', frozen_quote_amount: '0',
    settled_quote_amount: '10', refunded_quote_amount: '15',
  }
  const record = mapNewCoinSubscription(raw)
  assert.equal(record.settlementMode, 'manual_distribution')
  assert.equal(record.frozenQuoteAmountText, '0')
  assert.equal(record.settledQuoteAmountText, '10')
  assert.equal(record.refundedQuoteAmountText, '15')
  assert.throws(() => mapNewCoinSubscription({ ...raw, refunded_quote_amount: 15 }), NewCoinContractError)
  const legacy = mapNewCoinSubscription({ ...raw, settlement_mode: 'legacy_instant', settled_quote_amount: null, refunded_quote_amount: null })
  assert.equal(legacy.refundedQuoteAmountText, undefined)
})
