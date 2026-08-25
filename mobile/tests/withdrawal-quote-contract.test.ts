import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  calculateWithdrawalFee,
  isWithdrawalDecimalString,
  maximumQuotedWithdrawalAmount,
  normalizeWithdrawalPreviewAmount,
  withdrawalQuoteAmountsAreConsistent,
  type WithdrawalFeeTier,
} from '../src/core/withdrawalQuote.ts'

const tiers: WithdrawalFeeTier[] = [
  { minAmount: 10, maxAmount: 100, feeRatePercent: 1 },
  { minAmount: 100, feeRatePercent: 0.5 },
]

test('提现阶梯使用左闭右开边界，并由开放尾档覆盖上界', () => {
  assert.equal(calculateWithdrawalFee(9.999, 3, tiers), 3)
  assert.equal(calculateWithdrawalFee(10, 3, tiers), 0.1)
  assert.ok(Math.abs(calculateWithdrawalFee(99.999, 3, tiers) - 0.99999) < 1e-12)
  assert.equal(calculateWithdrawalFee(100, 3, tiers), 0.5)
  assert.equal(calculateWithdrawalFee(10_000, 3, tiers), 50)

  const precisionTiers: WithdrawalFeeTier[] = [
    { minAmount: 1, maxAmount: 10, feeRatePercent: 2 },
    { minAmount: 10, feeRatePercent: 3 },
  ]
  assert.equal(normalizeWithdrawalPreviewAmount(9.999, 2), 9.99)
  assert.equal(calculateWithdrawalFee(9.999, 0.25, precisionTiers, 2), 0.19)
  assert.equal(calculateWithdrawalFee(10, 0.25, precisionTiers, 2), 0.3)
  assert.equal(maximumQuotedWithdrawalAmount(10.18, 0.25, precisionTiers, 2), 9.99)
})

test('无命中阶梯回退固定费，全部金额同时预留本金与费用', () => {
  assert.equal(calculateWithdrawalFee(5, 2, tiers), 2)
  const maximum = maximumQuotedWithdrawalAmount(100, 2, [])
  assert.ok(Math.abs(maximum - 98) < 1e-9)
  assert.ok(maximum + calculateWithdrawalFee(maximum, 2, []) <= 100)

  const tierMaximum = maximumQuotedWithdrawalAmount(100, 3, tiers)
  assert.ok(tierMaximum + calculateWithdrawalFee(tierMaximum, 3, tiers) <= 100)
  assert.ok(tierMaximum > 98)

  const decreasingRateTiers: WithdrawalFeeTier[] = [
    { minAmount: 0, maxAmount: 100, feeRatePercent: 200 },
    { minAmount: 100, feeRatePercent: 0 },
  ]
  assert.equal(maximumQuotedWithdrawalAmount(150, 0, decreasingRateTiers), 150)
})

test('报价金额以 BigInt 对齐小数位，不经过 Number 丢失精度', () => {
  assert.equal(isWithdrawalDecimalString('9007199254740993.000000000000000000'), true)
  assert.equal(isWithdrawalDecimalString('1e3'), false)
  assert.equal(isWithdrawalDecimalString('-1'), false)
  assert.equal(withdrawalQuoteAmountsAreConsistent(
    '9007199254740993.000000000000000000',
    '0.000000000000000007',
    '9007199254740993',
    '9007199254740993.000000000000000007',
  ), true)
  assert.equal(withdrawalQuoteAmountsAreConsistent('10', '0.3', '9.7', '10.3'), false)
  assert.equal(withdrawalQuoteAmountsAreConsistent('10', '0.3', '10', '10.300000000000000001'), false)
  assert.equal(withdrawalQuoteAmountsAreConsistent('0', '0', '0', '0'), false)
})

test('mobile 只用服务端 quote 派生值提交并在确认层展示同一快照', () => {
  const api = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
  const view = readFileSync(new URL('../src/views/WithdrawView.vue', import.meta.url), 'utf8')

  assert.match(api, /withdraw_fee_tiers\?: BackendWithdrawalFeeTier/)
  assert.match(api, /\/wallet\/withdrawals\/quote/)
  assert.match(api, /quote_id: input\.quote\.quoteId/)
  assert.match(api, /idempotency_key: createWithdrawalIdempotencyKey\(input\.quote\.quoteId\)/)
  assert.match(api, /amount: input\.quote\.amount/)
  assert.match(api, /fee: input\.quote\.fee/)
  assert.match(api, /assertWithdrawalContract\(input\.quote, submitted\)/)
  assert.match(api, /withdrawalQuoteAmountsAreConsistent\(/)
  assert.match(api, /typeof value !== 'string'/)
  assert.match(view, /fetchWithdrawalQuote\(/)
  assert.match(view, /quote: quote\.value/)
  assert.match(view, /\{\{ quote\.amount \}\}/)
  assert.match(view, /\{\{ quote\.fee \}\}/)
  assert.match(view, /\{\{ quote\.net \}\}/)
  assert.match(view, /\{\{ quote\.totalReserved \}\}/)
  assert.match(view, /if \(quoting\.value \|\| submitting\.value\) return/)
  assert.doesNotMatch(view, /Number\(quote\.value\?\./)
  assert.doesNotMatch(view, /Number\(authorized\.totalReserved\)/)
  assert.doesNotMatch(view, /formatAmount\(quoted/)
  assert.doesNotMatch(view, /submitWithdrawal\(\{[\s\S]*?fee:\s*previewFee/)
})
