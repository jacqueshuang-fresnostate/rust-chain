import assert from 'node:assert/strict'
import test from 'node:test'
import { createServer } from 'vite'
import {
  decimalAdd, decimalDivide, decimalMultiply, decimalTextFromBoundary, decimalWithinRange,
  normalizeDecimalText, positiveDecimalInput, type DecimalText,
} from '../src/core/decimal.ts'
import { formatAmount, formatDateTime, formatExactAmount, formatFiat, formatPrice } from '../src/core/format.ts'
import { assertSafeJsonNumbers, normalizeTimestamp, requiredId, requiredSafeInteger } from '../src/core/numeric.ts'
import { createApiHttpClient } from '../src/core/apiRequest.ts'
import { walletAvailable, walletFrozen, walletTotal } from '../src/core/walletAmounts.ts'
import type { WalletAccount } from '../src/core/types.ts'
import { marginClosePreviewText } from '../src/core/marginClose.ts'

const LARGE = '9007199254740993.000000000000000001'
const ATOM = '0.000000000000000001'
const NOW = 1_786_307_400_000

test('bounded decimal operations reject forged/huge operands and unsafe numeric fallbacks', () => {
  assert.equal(decimalAdd(normalizeDecimalText(LARGE), normalizeDecimalText(ATOM)), '9007199254740993.000000000000000002')
  assert.equal(decimalMultiply(normalizeDecimalText(ATOM), normalizeDecimalText(ATOM)), `0.${'0'.repeat(35)}1`)
  assert.equal(decimalDivide(normalizeDecimalText('1'), normalizeDecimalText('3'), 36), `0.${'3'.repeat(36)}`)
  for (const value of ['1e999999999', 'NaN', 'Infinity', '', '0x10', '1'.repeat(1025), `0.${'0'.repeat(1000)}1`]) {
    assert.throws(() => normalizeDecimalText(value))
    assert.throws(() => decimalAdd(value as DecimalText, normalizeDecimalText('1')))
  }
  assert.throws(() => decimalMultiply(normalizeDecimalText('9'.repeat(100)), normalizeDecimalText('10')))
  assert.throws(() => decimalDivide(normalizeDecimalText('1'), normalizeDecimalText('1'), Number.MAX_SAFE_INTEGER))
  assert.equal(decimalTextFromBoundary(Number.MAX_SAFE_INTEGER + 1), null)
  assert.equal(decimalTextFromBoundary(Number.MIN_VALUE), null)
  assert.equal(positiveDecimalInput('99999999999999999999.999999999999999999'), '99999999999999999999.999999999999999999')
  assert.equal(positiveDecimalInput('100000000000000000000'), null)
  assert.equal(positiveDecimalInput('1.0000000000000000001'), null)
  assert.equal(positiveDecimalInput('1.0000000000000000000'), '1')
  assert.equal(decimalWithinRange(normalizeDecimalText('1'), { maximum: 'invalid' }), false)
  assert.equal(decimalWithinRange(normalizeDecimalText('1'), { available: null }), false)
  assert.equal(decimalWithinRange(normalizeDecimalText('1'), { available: Number.MAX_SAFE_INTEGER + 1 }), false)
})

test('financial formatting retains every confirmation digit and never prints tiny nonzero as zero', () => {
  assert.equal(marginClosePreviewText(normalizeDecimalText(LARGE), 100), LARGE)
  assert.equal(marginClosePreviewText(normalizeDecimalText(ATOM), 1), '0.00000000000000000001')
  assert.equal(marginClosePreviewText(normalizeDecimalText(`-${ATOM}`), 50), '-0.0000000000000000005')
  assert.equal(marginClosePreviewText(null, 50), null)
  assert.equal(formatExactAmount(LARGE), '9,007,199,254,740,993.000000000000000001')
  assert.equal(formatExactAmount(ATOM), ATOM)
  for (const format of [formatAmount, formatPrice, formatFiat]) {
    assert.match(format(ATOM), /0\.000000000000000001/)
    assert.match(format(`-${ATOM}`), /-.*0\.000000000000000001/)
    assert.equal(format('invalid'), '--')
    assert.equal(format(null), '--')
  }
  assert.match(formatFiat(LARGE), /9,007,199,254,740,993\.00/)
  assert.equal(formatDateTime(Number.MAX_SAFE_INTEGER), '--')
})

test('IDs/counts/time reject rounding, coercion, fractional epochs and date overflow', () => {
  assert.equal(requiredId('9007199254740991'), Number.MAX_SAFE_INTEGER)
  for (const value of ['9007199254740993', Number.MAX_SAFE_INTEGER + 1, 1.5, '1.0', '1e3', true, null, '', -1]) {
    assert.throws(() => requiredId(value))
  }
  assert.equal(requiredSafeInteger(0), 0)
  assert.equal(normalizeTimestamp(1_786_307_400), NOW)
  for (const value of [1.1, '1.1', Number.MAX_SAFE_INTEGER, Infinity, -1]) {
    assert.throws(() => normalizeTimestamp(value))
  }
  assert.throws(() => assertSafeJsonNumbers({ rows: [{ id: Number.MAX_SAFE_INTEGER + 1 }] }))
  assert.doesNotThrow(() => assertSafeJsonNumbers({ amount: LARGE, chart: 0.5, count: 0 }))
})

test('shared HTTP transport rejects unsafe request/response JSON before passthrough DTOs can act', async () => {
  let requests = 0
  const http = createApiHttpClient({
    adapter: async (config) => {
      requests++
      return { data: '{"rows":[{"id":9007199254740993}]}', status: 200, statusText: 'OK', headers: {}, config }
    },
  })
  await assert.rejects(http.get('/unsafe'), /invalid id|unsafe JSON number/)
  await assert.rejects(http.post('/unsafe', { asset_id: Number.MAX_SAFE_INTEGER + 1 }), /invalid asset_id|unsafe JSON number/)
  assert.equal(requests, 1)
  await assert.rejects(http.get('/fractional-count', { params: { limit: 1.5 } }), /invalid limit/)
  assert.equal(requests, 1)
})

test('wallet totals use exact bucket text rather than legacy rounded display values', () => {
  const account: WalletAccount = {
    assetId: 1, symbol: 'USDT', available: Number(LARGE), frozen: Number(ATOM), locked: Number(ATOM),
    availableText: normalizeDecimalText(LARGE), frozenText: normalizeDecimalText(ATOM), lockedText: normalizeDecimalText(ATOM),
  }
  assert.equal(walletAvailable(account), LARGE)
  assert.equal(walletFrozen(account), '0.000000000000000002')
  assert.equal(walletTotal(account), '9007199254740993.000000000000000003')
  assert.equal(walletTotal(), '0')
  assert.throws(() => walletTotal({ ...account, availableText: undefined }), /invalid wallet available/)
})

test('real swap/earn/loan/prediction adapters preserve >2^53 and 1e-18 through models and payloads', async (context) => {
  const server = await createServer({ appType: 'custom', logLevel: 'silent', server: { middlewareMode: true } })
  context.after(async () => server.close())
  const { client } = await server.ssrLoadModule('/src/api/client.ts')
  const swap = await server.ssrLoadModule('/src/api/swap.ts')
  const earn = await server.ssrLoadModule('/src/api/earn.ts')
  const loan = await server.ssrLoadModule('/src/api/loan.ts')
  const prediction = await server.ssrLoadModule('/src/api/prediction.ts')
  const pairs = [{
    id: 1, from_asset_id: 2, to_asset_id: 3, from_asset_symbol: 'USDT', to_asset_symbol: 'BTC',
    min_amount: ATOM, max_amount: LARGE, fee_rate: ATOM,
  }]
  const subscription: Record<string, unknown> = {
    id: 1, product_id: 2, asset_id: 3, amount: LARGE, apr_rate: ATOM,
    term_days: 7, status: 'active', subscribed_at: NOW, matures_at: NOW,
  }
  const order: Record<string, unknown> = {
    id: 1, product_id: 2, amount: LARGE, interest_rate: ATOM, term_days: 7,
    collateral_amount: ATOM, interest_amount: ATOM, repayment_amount: LARGE, created_at: NOW,
  }
  const quote: Record<string, unknown> = {
    quote_id: 'exact-quote', convert_pair_id: 1, from_amount: LARGE, to_amount: ATOM,
    rate: ATOM, fee_amount: ATOM, expires_at: NOW,
  }
  const product: Record<string, unknown> = {
    id: 1, asset_id: 2, asset_symbol: 'USDT', term_days: 7, apr_rate: ATOM,
    min_subscribe: ATOM, max_subscribe: LARGE,
    min_amount: ATOM, max_amount: LARGE, interest_rate: ATOM, min_kyc_level: 0,
  }
  let lastPayload: Record<string, unknown> = {}
  client.get = async (url: string) => {
    if (url.endsWith('/convert/pairs')) return { data: { pairs } }
    if (url.endsWith('/earn/subscriptions')) return { data: { subscriptions: [subscription] } }
    if (url.endsWith('/loan/orders')) return { data: { orders: [order] } }
    if (url.endsWith('/products')) return { data: { products: [product] } }
    if (url.endsWith('/convert/orders')) return { data: { orders: [{
      id: 1, from_asset_id: 2, to_asset_id: 3, ...quote, created_at: NOW,
    }] } }
    throw new Error(`unexpected GET ${url}`)
  }
  client.post = async (url: string, payload: Record<string, unknown>) => {
    lastPayload = payload
    if (url.endsWith('/prediction/quotes')) return { data: {
      quote_id: 'prediction-quote', outcome: 'yes', asset_id: 3,
      stake_amount: LARGE, fee_amount: ATOM, shares: LARGE, theoretical_payout: LARGE, expires_at: NOW,
    } }
    return { data: quote }
  }
  const pair = (await swap.fetchConvertPairs({ force: true }))[0]
  assert.equal(pair.maxAmount, LARGE)
  const converted = await swap.requestConvertQuote(pair, LARGE)
  assert.equal(lastPayload.from_amount, LARGE)
  assert.equal(converted.fromAmount, LARGE)
  assert.equal(converted.toAmount, ATOM)
  assert.equal(converted.feeAmount, ATOM)
  assert.equal((await swap.fetchConvertOrders())[0].fromAmount, LARGE)
  await swap.confirmConvertQuote(converted.quoteId)
  assert.deepEqual(lastPayload, { quote_id: 'exact-quote' })
  assert.equal((await earn.fetchEarnProducts(50, { force: true }))[0].aprRate, ATOM)
  assert.equal((await earn.fetchEarnSubscriptions())[0].amount, LARGE)
  assert.equal((await loan.fetchLoanProducts(50, { force: true }))[0].maxAmount, LARGE)
  const bill = (await loan.fetchLoanOrders())[0]
  assert.equal(bill.repaymentAmount, LARGE)
  assert.equal(bill.interestAmount, ATOM)
  assert.equal(bill.collateralAmount, ATOM)
  const predicted = await prediction.requestPredictionQuote({ marketId: 1, assetId: 3, outcome: 'yes', stakeAmount: LARGE })
  assert.equal((lastPayload as Record<string, unknown>).stake_amount, LARGE)
  assert.equal(predicted.stakeAmount, LARGE)
  assert.equal(predicted.feeAmount, ATOM)
  assert.equal(predicted.shares, LARGE)
  for (const invalid of [0.1, Number(ATOM), Number(LARGE), '1e99999999', 'NaN', 'Infinity', '-1', '', `0.${'0'.repeat(1000)}1`, '1.0000000000000000001', '100000000000000000000']) {
    subscription.amount = invalid
    order.repayment_amount = invalid
    quote.fee_amount = invalid
    await assert.rejects(earn.fetchEarnSubscriptions(), /invalid/)
    await assert.rejects(loan.fetchLoanOrders(), /invalid/)
    await assert.rejects(swap.requestConvertQuote(pair, normalizeDecimalText('1')), /invalid/)
  }
  subscription.amount = LARGE
  subscription.id = '9007199254740993'
  await assert.rejects(earn.fetchEarnSubscriptions(), /invalid id/)
  await assert.rejects(loan.repayLoanOrder(Number.MAX_SAFE_INTEGER + 1), /invalid id/)
  await assert.rejects(earn.redeemEarnSubscription(Number.MAX_SAFE_INTEGER + 1), /invalid id/)
})
