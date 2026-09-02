import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import {
  financialAssetDisplayDigits,
  formatFinancialAmount,
} from '../src/core/financialDisplay.ts'

test('金融金额按资产显示精度四舍五入且不经过 IEEE-754', () => {
  const source = normalizeDecimalText('1134.331253942506787192')
  assert.equal(formatFinancialAmount(source, 'en-US', { assetSymbol: 'USDT' }), '1,134.33')
  assert.equal(formatFinancialAmount(normalizeDecimalText('-1134.335'), 'en-US', { assetSymbol: 'USDT' }), '-1,134.34')
  assert.equal(formatFinancialAmount(normalizeDecimalText('999.999'), 'en-US', {
    assetSymbol: 'USDT',
    minimumFractionDigits: 2,
  }), '1,000.00')
  assert.equal(source, '1134.331253942506787192')
})

test('非稳定币最多八位且接口的更低业务精度可以继续收紧', () => {
  assert.equal(financialAssetDisplayDigits('BTC'), 8)
  assert.equal(financialAssetDisplayDigits('BTC', 4), 4)
  assert.equal(formatFinancialAmount(normalizeDecimalText('1.234567895'), 'en-US', {
    assetSymbol: 'BTC',
  }), '1.2345679')
  assert.equal(formatFinancialAmount(normalizeDecimalText('1.23456'), 'en-US', {
    assetSymbol: 'BTC',
    precisionScale: 4,
  }), '1.2346')
})

test('极小非零金额显示阈值，零和极大整数保持准确', () => {
  assert.equal(formatFinancialAmount(normalizeDecimalText('0.000000001'), 'en-US', {
    assetSymbol: 'BTC',
  }), '<0.00000001')
  assert.equal(formatFinancialAmount(normalizeDecimalText('-0.000000001'), 'en-US', {
    assetSymbol: 'BTC',
  }), '>-0.00000001')
  assert.equal(formatFinancialAmount(normalizeDecimalText('-0'), 'en-US', { assetSymbol: 'USDT' }), '0')
  assert.equal(formatFinancialAmount(
    normalizeDecimalText('9007199254740993.129999999999999999'),
    'en-US',
    { assetSymbol: 'USDT' },
  ), '9,007,199,254,740,993.13')
})

test('非法显示精度和非法边界值不会产生误导文本', () => {
  assert.equal(formatFinancialAmount('not-a-decimal', 'en-US', { assetSymbol: 'USDT' }), '--')
  assert.throws(() => financialAssetDisplayDigits('BTC', 19), /precision/)
  assert.throws(() => formatFinancialAmount('1', 'en-US', {
    assetSymbol: 'USDT',
    minimumFractionDigits: 3,
  }), /minimum/)
})
