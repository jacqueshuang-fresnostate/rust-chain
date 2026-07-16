import test from 'node:test'
import assert from 'node:assert/strict'

import {
  historyKlineBars,
  klineLookbackMs,
  normalizeKlineInterval,
  normalizeKlineModule,
  parseRealtimeKline,
  resolveKlineTopic
} from '../src/components/chart/klineData.ts'

test('normalizes chart module, interval, topic, and lookback boundaries', () => {
  assert.equal(normalizeKlineModule('market'), 'spot')
  assert.equal(normalizeKlineModule('swap'), 'margin')
  assert.equal(normalizeKlineModule('second'), 'seconds')
  assert.equal(normalizeKlineInterval('15min'), '15m')
  assert.equal(normalizeKlineInterval('1day'), '1d')
  assert.equal(resolveKlineTopic('spot', 'feed:{symbol}:{interval}', 'BTC/USDT', '5min'), 'feed:BTC/USDT:5m')
  assert.equal(klineLookbackMs('4h'), 4 * 3_600_000 * 100)
})

test('maps history and realtime candles into a stable millisecond stream', () => {
  const bars = historyKlineBars([
    [1_700_000_060, '12', '13', '11', '12.5', '8'],
    [1_700_000_000, '10', '11', '9', '10.5', '7'],
    [1_700_000_060, '12', '14', '10', '13', '9'],
    ['invalid', '1', '2', '0.5', '1.5', '1']
  ])

  assert.deepEqual(bars, [
    { timestamp: 1_700_000_000_000, open: 10, high: 11, low: 9, close: 10.5, volume: 7 },
    { timestamp: 1_700_000_060_000, open: 12, high: 14, low: 10, close: 13, volume: 9 }
  ])
  assert.deepEqual(
    parseRealtimeKline({
      time: 1_700_000_120,
      openPrice: '13',
      highestPrice: '15',
      lowestPrice: '12',
      closePrice: '14',
      volume: '10',
      turnover: '140'
    }),
    {
      timestamp: 1_700_000_120_000,
      open: 13,
      high: 15,
      low: 12,
      close: 14,
      volume: 10,
      turnover: 140
    }
  )
  assert.equal(parseRealtimeKline({ time: 1_700_000_120, close: 0 }), null)
})
