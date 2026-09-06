import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { resolveMarketChartPriceFormat } from '../src/core/marketChart.ts'

const source = readFileSync(new URL('../src/components/LightweightMarketChart.vue', import.meta.url), 'utf8')

test('蜡烛和均线显式使用相同价格精度，不依赖两位小数默认值', () => {
  assert.match(source, /resolveMarketChartPriceFormat/)
  assert.match(source, /candles\?\.applyOptions\(\{ priceFormat \}\)/)
  assert.match(source, /ma5Series\?\.applyOptions\(\{ priceFormat \}\)/)
  assert.match(source, /ma10Series\?\.applyOptions\(\{ priceFormat \}\)/)
  assert.match(source, /ma20Series\?\.applyOptions\(\{ priceFormat \}\)/)
})

test('六位低价 OHLC 显示真实差值，成交量不决定价格刻度', () => {
  const points = [{ open: 0.096527, high: 0.393109, low: 0.000001, close: 0.103197, volume: 1.123456789123 }]
  const before = structuredClone(points)
  assert.deepEqual(resolveMarketChartPriceFormat(points), { type: 'price', precision: 6, minMove: 1e-6, base: 1e6 })
  assert.deepEqual(points, before)
})

test('小于一分的平价蜡烛按真实精度缩放，而不是默认 0.01 步长', () => {
  assert.equal(resolveMarketChartPriceFormat([{ open: 0.001234, high: 0.001234, low: 0.001234, close: 0.001234 }]).minMove, 1e-6)
})

test('科学计数法和十八位小数保留非零价格，base 避免倒数精度误差', () => {
  for (const [price, precision] of [[1e-8, 8], [1.23e-8, 10], [1e-18, 18]] as const) {
    const format = resolveMarketChartPriceFormat([{ open: price, high: price, low: price, close: price }])
    assert.equal(format.precision, precision)
    assert.equal(format.base, 10 ** precision)
    assert.equal(format.minMove, 10 ** -precision)
  }
})

test('实时更细的价格会提高精度，切换数据集重新计算，空值与非法值不污染格式', () => {
  const first = { open: 0.1, high: 0.12, low: 0.09, close: 0.11 }
  const next = { ...first, close: 0.111111 }
  assert.equal(resolveMarketChartPriceFormat([first]).precision, 2)
  assert.equal(resolveMarketChartPriceFormat([first, next]).precision, 6)
  assert.equal(resolveMarketChartPriceFormat([first]).precision, 2)
  assert.equal(resolveMarketChartPriceFormat([]).precision, 2)
  assert.equal(resolveMarketChartPriceFormat([{ open: NaN, high: Infinity, low: 0, close: -1 }]).precision, 2)
})
