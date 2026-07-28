import assert from 'node:assert/strict'
import test from 'node:test'
import { parseMarketSocketFrame, tickerSubscriptionFrame } from '../src/api/marketSocketProtocol.ts'

test('market WebSocket subscription payload matches the backend public ticker contract', () => {
  assert.deepEqual(JSON.parse(tickerSubscriptionFrame('BTCUSDT')), {
    op: 'subscribe',
    channel: 'ticker',
    symbol: 'BTCUSDT',
  })
})

test('market WebSocket parser preserves confirmation, ticker, and text heartbeat frames', () => {
  assert.deepEqual(
    parseMarketSocketFrame('{"type":"subscribed","channel":"public:ticker:BTCUSDT"}'),
    { type: 'subscribed', channel: 'public:ticker:BTCUSDT' },
  )
  assert.deepEqual(
    parseMarketSocketFrame('{"symbol":"BTC-USDT","last_price":"61234.5","observed_at":1720000000000}'),
    { type: 'ticker', symbol: 'BTC-USDT', lastPrice: 61234.5, observedAt: 1720000000000 },
  )
  assert.deepEqual(parseMarketSocketFrame('pong'), { type: 'pong' })
  assert.equal(parseMarketSocketFrame('{"type":"error","code":"invalid_request"}'), null)
  assert.equal(parseMarketSocketFrame('not-json'), null)
})
