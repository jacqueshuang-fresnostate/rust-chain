import assert from 'node:assert/strict'
import test from 'node:test'

import { decimalAdd, normalizeDecimalText } from '../src/core/decimal.ts'
import {
  createSecondsFinancialPresentation,
  createSecondsOrderReviewSnapshot,
} from '../src/core/secondsFinancial.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

function presentation() {
  return createSecondsFinancialPresentation({
    locale: () => 'en-US',
    exactByOrderId: new Map(),
    normalizeSymbol: (symbol) => symbol.replace(/[-_/\s]/g, '').toUpperCase(),
    liveTickerFor: () => undefined,
    marketTickerFor: () => undefined,
    selectedSymbol: () => 'BTCUSDT',
    selectedCandleClose: () => null,
    translate: (key) => key,
  })
}

test('0.4 remains a 40% net rate and a 100-unit win totals 140', () => {
  const review = createSecondsOrderReviewSnapshot({
    productId: 7,
    cycleId: 60,
    symbol: 'BTC-USDT',
    stakeAssetId: 1,
    stakeAssetSymbol: 'USDT',
    durationSeconds: 60,
    direction: 'up',
    stakeAmount: '100',
    minimumStake: '1',
    maximumStake: '1000',
    available: '1000',
    payoutRate: '0.4',
    referencePrice: '60000',
    idempotencyKey: 'seconds-net-rate-40',
  })

  assert.ok(review)
  assert.equal(review.payoutRate, '0.4')
  assert.equal(review.estimatedProfit, '40')
  assert.equal(presentation().formatPayoutRate(review.payoutRate, 0), '40')
  assert.equal(
    decimalAdd(normalizeDecimalText('100'), review.estimatedProfit),
    '140',
  )
})

test('historical 1.4 order snapshots stay visible and are not client-normalized', () => {
  const financial = presentation()
  const historicalOrder = {
    id: 77,
    result: 'win',
    stakeAmount: '100',
    stakeAmountText: '100',
    payoutRate: '1.4',
    payoutRateText: '1.4',
  }

  assert.equal(financial.orderFinancials(historicalOrder).payoutRate, '1.4')
  assert.equal(financial.formatPayoutRate('1.4', 0), '140')
  assert.deepEqual(financial.profitLoss(historicalOrder), {
    kind: 'profit',
    amount: '140',
  })
})

test('mobile copy names net profit and states that principal is excluded', () => {
  assert.match(zhCN.seconds.returnRate, /净收益率/)
  assert.match(zhCN.seconds.payoutRate, /不含本金/)
  assert.match(zhCN.seconds.estimatedProfit, /净收益/)
  assert.match(en.seconds.returnRate, /Net profit/)
  assert.match(en.seconds.payoutRate, /principal excluded/)
  assert.match(en.seconds.estimatedProfit, /net profit/)
})
