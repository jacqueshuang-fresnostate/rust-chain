import assert from 'node:assert/strict'
import test from 'node:test'
import { mapMarketTicker } from '../src/core/marketMapper.ts'

test('后端未返回涨跌幅时由开盘价和最新价计算', () => {
  const ticker = mapMarketTicker(
    {
      symbol: 'RE_USDT',
      logo_url: 'https://cdn.example.test/pairs/re-usdt.png',
      base_logo_url: 'https://cdn.example.test/assets/re.png',
      quote_logo_url: 'https://cdn.example.test/assets/usdt.png',
      base_asset: 'RE',
      quote_asset: 'USDT',
    },
    {
      last_price: '0.59379',
      open_24h: '0.57549',
      price_change_24h: '999999',
      high_24h: '0.6014',
      low_24h: '0.5682',
      volume_24h: '1244093',
      observed_at: 1_784_000_000,
    },
  )

  assert.equal(ticker.symbol, 'RE/USDT')
  assert.ok(Math.abs(ticker.changePercent - ((0.59379 - 0.57549) / 0.57549) * 100) < 0.000001)
  assert.equal(ticker.observedAt, 1_784_000_000_000)
  assert.equal(ticker.iconUrl, 'https://cdn.example.test/pairs/re-usdt.png')
  assert.equal(ticker.baseIconUrl, 'https://cdn.example.test/assets/re.png')
  assert.equal(ticker.quoteIconUrl, 'https://cdn.example.test/assets/usdt.png')
})

test('后端返回 Bitget 现货涨跌幅时优先使用该字段', () => {
  const ticker = mapMarketTicker(
    { symbol: 'BTC_USDT' },
    {
      last_price: '63670',
      price_change_24h: '59.99',
      price_change_percent_24h: '-0.79700',
    },
  )

  assert.equal(ticker.changePercent, -0.797)
  assert.ok(Math.abs(ticker.openPrice - 63670 / (1 - 0.797 / 100)) < 0.000001)
})

test('后端返回零涨跌幅时不回退到绝对差值推导', () => {
  const ticker = mapMarketTicker(
    { symbol: 'BTC_USDT' },
    {
      last_price: '63670',
      price_change_24h: '59.99',
      price_change_percent_24h: '0',
    },
  )

  assert.equal(ticker.changePercent, 0)
  assert.equal(ticker.openPrice, 63670)
})
