import assert from 'node:assert/strict'
import test from 'node:test'

import {
  estimateLoanInterest,
  estimateLoanOrderInterest,
  estimateLoanOrderRepayment,
  estimateLoanRepayment,
  loanAmountRangeError,
  normalizeLoanAmountInput,
  parseLoanNumber,
} from '../src/utils/loan.ts'

test('loan page estimates interest and repayment from backend decimal strings', () => {
  const product = {
    interest_rate: '0.02',
    min_amount: '100',
    max_amount: '2000',
  }

  assert.equal(parseLoanNumber('1,000.50'), 1000.5)
  assert.equal(normalizeLoanAmountInput(' 1,000.50 '), '1000.50')
  assert.equal(estimateLoanInterest('1000', product), 20)
  assert.equal(estimateLoanRepayment('1000', product), 1020)
})

test('loan page estimates backend alias and object decimal rate fields', () => {
  assert.equal(estimateLoanInterest('1000', { interestRate: '0.02', min_amount: '100' }), 20)
  assert.equal(estimateLoanInterest('1000', { rate: '0.02', min_amount: '100' }), 20)
  assert.equal(estimateLoanInterest('1000', { interest_rate: { value: '0.02' }, min_amount: '100' }), 20)
  assert.equal(estimateLoanInterest('1000', { interest_rate: { int_val: '2', scale: '2' }, min_amount: '100' }), 20)
})

test('loan page validates configured amount range before submit', () => {
  const product = {
    interest_rate: '0.02',
    min_amount: '100',
    max_amount: '2000',
  }

  assert.equal(loanAmountRangeError('', product), 'invalid')
  assert.equal(loanAmountRangeError('0', product), 'invalid')
  assert.equal(loanAmountRangeError('99.99', product), 'below_min')
  assert.equal(loanAmountRangeError('2000.01', product), 'above_max')
  assert.equal(loanAmountRangeError('1000', product), null)
})

test('loan order estimates payable amount for disbursed full-term orders', () => {
  const order = {
    amount: '10000',
    interest_rate: '0.02',
    interest_calculation_mode: 'full_term',
    term_days: 30,
    status: 'disbursed',
    interest_amount: '0',
    repayment_amount: '0',
  }

  assert.equal(estimateLoanOrderInterest(order), 200)
  assert.equal(estimateLoanOrderRepayment(order), 10200)
})

test('loan order actual-days repayment charges at least one day', () => {
  const now = Date.UTC(2026, 5, 17, 7, 30)
  const order = {
    amount: '10000',
    interest_rate: '0.02',
    interest_calculation_mode: 'actual_days',
    term_days: 30,
    disbursed_at: now,
    status: 'disbursed',
    interest_amount: '0',
    repayment_amount: '0',
  }

  const expectedInterest = (10000 * 0.02) / 30
  assert.ok(Math.abs(estimateLoanOrderInterest(order, now) - expectedInterest) < 1e-9)
  assert.ok(Math.abs(estimateLoanOrderRepayment(order, now) - (10000 + expectedInterest)) < 1e-9)
})

test('loan order uses settled repayment fields after repayment', () => {
  const order = {
    amount: '10000',
    interest_rate: '0.02',
    interest_calculation_mode: 'actual_days',
    term_days: 30,
    status: 'repaid',
    interest_amount: '6.66',
    repayment_amount: '10006.66',
  }

  assert.equal(estimateLoanOrderInterest(order), 6.66)
  assert.equal(estimateLoanOrderRepayment(order), 10006.66)
})
