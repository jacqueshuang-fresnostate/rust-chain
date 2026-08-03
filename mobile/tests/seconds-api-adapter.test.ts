import assert from 'node:assert/strict'
import test from 'node:test'
import { mapSecondsOrder } from '../src/core/secondsOrder.ts'

test('秒合约订单适配器保留锁单赔率、成交价、结算价和 opened 状态', () => {
  assert.deepEqual(mapSecondsOrder({
    id: 42,
    symbol: 'BTC/USDT',
    stake_asset_symbol: 'usdt',
    direction: 'down',
    stake_amount: '100.000000000000000000',
    duration_seconds: 30,
    payout_rate: '0.92400000',
    entry_price: '63080.000000000000000000',
    settlement_price: null,
    status: 'opened',
    result: null,
    expires_at: 1_722_000_030,
    created_at: 1_722_000_000,
  }), {
    id: 42,
    symbol: 'BTC/USDT',
    stakeAssetSymbol: 'USDT',
    direction: 'down',
    stakeAmount: 100,
    durationSeconds: 30,
    payoutRate: .924,
    entryPrice: 63080,
    settlementPrice: undefined,
    status: 'opened',
    result: undefined,
    expiresAt: 1_722_000_030_000,
    createdAt: 1_722_000_000_000,
  })
})

test('秒合约订单适配器保留已结算真实结果', () => {
  const order = mapSecondsOrder({
    id: 43,
    symbol: 'ETH_USDT',
    stake_asset_symbol: 'USDT',
    direction: 'up',
    stake_amount: '20',
    duration_seconds: 60,
    payout_rate: '0.8',
    entry_price: '3100',
    settlement_price: '3120',
    status: 'settled',
    result: 'win',
    expires_at: 1_722_000_060_000,
    created_at: 1_722_000_000_000,
  })

  assert.equal(order.payoutRate, .8)
  assert.equal(order.entryPrice, 3100)
  assert.equal(order.settlementPrice, 3120)
  assert.equal(order.result, 'win')
})

test('秒合约订单适配器不会把未知后端方向静默伪装为看涨', () => {
  assert.throws(() => mapSecondsOrder({ direction: 'sideways' }), /invalid direction/)
})
