import assert from 'node:assert/strict'
import test from 'node:test'
import { formatDateTime, formatPrice, normalizeSymbol, shortAddress, splitSymbol } from '../src/core/format.ts'

test('交易对标准化可兼容下划线、横线和斜杠', () => {
  assert.equal(normalizeSymbol('btc_usdt'), 'BTCUSDT')
  assert.deepEqual(splitSymbol('BTC_USDT'), { base: 'BTC', quote: 'USDT' })
  assert.deepEqual(splitSymbol('ETHUSDC'), { base: 'ETH', quote: 'USDC' })
})

test('价格与地址格式在移动端保持稳定', () => {
  assert.equal(formatPrice(0.59379), '0.5938')
  assert.equal(formatPrice(64_125), '64,125.00')
  assert.equal(shortAddress('0xeb433df22ca8e078e10e8e193f3931ce6aab158e', 8, 6), '0xeb433d...ab158e')
})

test('时间戳可兼容秒和毫秒格式', () => {
  assert.equal(formatDateTime(0), '--')
  assert.equal(formatDateTime(1_704_067_200), formatDateTime(1_704_067_200_000))
})
