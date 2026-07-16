import assert from 'node:assert/strict'
import test from 'node:test'
import { mapMarketTicker } from '../src/core/marketMapper.ts'

test('行情涨跌幅始终由开盘价和最新价计算', () => {
  const ticker = mapMarketTicker(
    { symbol: 'RE_USDT', base_asset: 'RE', quote_asset: 'USDT' },
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
})
