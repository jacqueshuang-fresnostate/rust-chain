import assert from 'node:assert/strict'
import test from 'node:test'
import {
  decimalAdd,
  decimalCompare,
  decimalDivide,
  decimalMinimum,
  decimalPortion,
  decimalWithinRange,
  formatDecimalText,
  normalizeDecimalText,
  positiveDecimalInput,
} from '../src/core/decimal.ts'
import {
  commitSpotCancelAllResult,
  createOrdersRequestLifecycle,
  spotCancelAllOutcome,
} from '../src/core/ordersRequest.ts'
import { createObjectUrlRegistry } from '../src/core/objectUrlRegistry.ts'
import {
  isKycSubmissionLocked,
  kycStatusPresentation,
  quickRechargeStatusPresentation,
} from '../src/core/financialEnumPresentation.ts'
import {
  formatWalletLedgerDecimal,
  mapWalletLedgerResponse,
} from '../src/core/walletLedger.ts'

test('DecimalText preserves 18-decimal values and integers beyond Number safe range', () => {
  const atom = normalizeDecimalText('0.000000000000000001')
  const large = normalizeDecimalText('9007199254740993.000000000000000001')
  assert.equal(formatDecimalText(atom, 'en-US', { maximumFractionDigits: 18 }), '0.000000000000000001')
  assert.equal(formatDecimalText(large, 'en-US', { maximumFractionDigits: 18 }), '9,007,199,254,740,993.000000000000000001')
  assert.equal(decimalCompare(large, normalizeDecimalText('9007199254740992')), 1)
  assert.equal(decimalAdd(atom, atom), normalizeDecimalText('0.000000000000000002'))
  assert.equal(decimalDivide(normalizeDecimalText('1'), normalizeDecimalText('3'), 18), normalizeDecimalText('0.333333333333333333'))
})

test('financial input range and percentage shortcuts remain exact beyond IEEE-754', () => {
  const amount = positiveDecimalInput('9007199254740993.000000000000000001')
  assert.ok(amount)
  assert.equal(decimalWithinRange(amount, {
    minimum: '0.000000000000000001',
    maximum: '9007199254740993.000000000000000001',
    available: '9007199254740993.000000000000000002',
  }), true)
  assert.equal(decimalWithinRange(amount, { available: '9007199254740993' }), false)
  assert.equal(decimalPortion(normalizeDecimalText('0.000000000000000004'), 25), '0.000000000000000001')
  assert.equal(decimalMinimum('9007199254740993', '9007199254740992.999999999999999999'), '9007199254740992.999999999999999999')
})

test('wallet ledger keeps source decimals while applying a separate asset display policy', () => {
  const result = mapWalletLedgerResponse({
    entries: [{
      id: 1,
      account_type: 'spot',
      symbol: 'USDT',
      change_type: 'deposit',
      category: 'funding',
      amount: '0.000000000000000001',
      fee: '0.000000000000000001',
      balance_after: '9007199254740993.000000000000000001',
      precision_scale: 18,
      created_at: 1_786_307_400,
    }],
    page: { number: 0, size: 30, total_elements: 1, total_pages: 1 },
  })
  const entry = result.entries[0]
  assert.equal(entry.amount, '0.000000000000000001')
  assert.equal(formatWalletLedgerDecimal(entry.amount, 'en-US', entry.precisionScale, entry.symbol), '<0.01')
  assert.equal(formatWalletLedgerDecimal(entry.balanceAfter, 'en-US', entry.precisionScale, entry.symbol), '9,007,199,254,740,993')
  assert.equal(entry.balanceAfter, '9007199254740993.000000000000000001')

  assert.throws(() => mapWalletLedgerResponse({
    entries: [{
      id: 2,
      account_type: 'spot',
      symbol: 'USDT',
      change_type: 'deposit',
      category: 'funding',
      amount: '1.000000000000000000',
      fee: '0.000000000000000000',
      balance_after: '2.000000000000000000',
      created_at: 1_786_307_400,
    }],
    page: { number: 0, size: 30, total_elements: 1, total_pages: 1 },
  }), /precision_scale/)
})

test('orders lifecycle aborts superseded generations and ignores their late completion', async () => {
  const lifecycle = createOrdersRequestLifecycle()
  let resolveOld: ((value: string) => void) | undefined
  let oldSignal: AbortSignal | undefined
  const oldRequest = lifecycle.load({ sessionGeneration: 7, market: 'margin', state: 'history' }, (signal) => {
    oldSignal = signal
    return new Promise<string>((resolve) => { resolveOld = resolve })
  })
  const current = await lifecycle.load(
    { sessionGeneration: 8, market: 'spot', state: 'current' },
    async () => 'spot-current',
  )
  assert.equal(oldSignal?.aborted, true)
  assert.deepEqual(current, {
    state: 'loaded',
    snapshot: { sessionGeneration: 8, market: 'spot', state: 'current' },
    value: 'spot-current',
  })
  resolveOld?.('margin-history')
  assert.deepEqual(await oldRequest, { state: 'stale' })
  lifecycle.stop()
})

test('spot batch outcome distinguishes success, partial and total failure', () => {
  assert.equal(spotCancelAllOutcome({ orders: [{ id: '1' }], failures: [] }).kind, 'success')
  assert.deepEqual(spotCancelAllOutcome({
    orders: [{ id: '1' }],
    failures: [{ id: '2', code: 'ORDER_BUSY', message: 'busy' }],
  }), {
    kind: 'partial',
    succeeded: 1,
    failed: 1,
    failureDetails: ['2: busy'],
  })
  assert.equal(spotCancelAllOutcome({
    orders: [],
    failures: [{ id: '2', code: 'ORDER_BUSY', message: 'busy' }],
  }).kind, 'failure')

  const committed = commitSpotCancelAllResult(
    [{ id: '1', symbol: 'BTC/USDT' }, { id: '2', symbol: 'ETH/USDT' }],
    {
      orders: [{ id: '1' }],
      failures: [{ id: '2', code: 'ORDER_BUSY', message: 'busy' }],
    },
  )
  assert.deepEqual(committed.remainingOrders, [{ id: '2', symbol: 'ETH/USDT' }])
  assert.equal(committed.outcome.kind, 'partial')
})

test('KYC preview registry revokes replacement and all live URLs exactly once', () => {
  let sequence = 0
  const revoked: string[] = []
  const registry = createObjectUrlRegistry<'front' | 'back'>({
    createObjectURL: () => `blob:test-${++sequence}`,
    revokeObjectURL: (url) => { revoked.push(url) },
  })
  registry.replace('front', new Blob(['a']))
  registry.replace('front', new Blob(['b']))
  registry.replace('back', new Blob(['c']))
  assert.deepEqual(revoked, ['blob:test-1'])
  registry.clearAll()
  assert.deepEqual(revoked, ['blob:test-1', 'blob:test-2', 'blob:test-3'])
})

test('unknown financial enums preserve raw source and never impersonate pending', () => {
  const kyc = kycStatusPresentation('future_status_v2')
  assert.equal(kyc.known, false)
  assert.equal(kyc.source, 'future_status_v2')
  assert.equal(kyc.translationKey, 'common.unknownStatusWithSource')
  assert.equal(isKycSubmissionLocked('future_status_v2'), true)
  assert.equal(quickRechargeStatusPresentation('completed').tone, 'positive')
})
