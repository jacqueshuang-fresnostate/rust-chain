import assert from 'node:assert/strict'
import test from 'node:test'
import {
  activeSecondsOrders,
  historicalSecondsOrders,
  isActiveSecondsOrder,
  mapSecondsOrder,
  mergeSecondsOrderReconciliation,
  secondsOrderEstimatedProfit,
  secondsOrderProgress,
  secondsOrderRemainingMs,
  secondsOrderStatusPresentation,
  upsertSecondsOrder,
  type SecondsOrder,
} from '../src/core/secondsOrder.ts'

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

test('活动秒合约保留全量订单并按各自时间和赔率独立计算', () => {
  const order = (id: number, status: string, createdAt: number, expiresAt: number): SecondsOrder => ({
    id,
    symbol: id === 1 ? 'BTCUSDT' : 'ETHUSDT',
    stakeAssetSymbol: 'USDT',
    direction: id === 1 ? 'up' : 'down',
    stakeAmount: id * 10,
    durationSeconds: 60,
    payoutRate: id === 1 ? .8 : .9,
    status,
    createdAt,
    expiresAt,
  })
  const now = 1_720_000_030_000
  const first = order(1, 'opened', 1_720_000_000_000, 1_720_000_060_000)
  const second = order(2, 'ACTIVE', 1_720_000_020_000, 1_720_000_080_000)
  const pending = order(3, 'pending', 1_720_000_030_000, 1_720_000_090_000)
  const settled = order(4, 'settled', 1_720_000_000_000, 1_720_000_010_000)

  assert.deepEqual(activeSecondsOrders([first, settled, second, pending]).map(({ id }) => id), [1, 2, 3])
  assert.equal(secondsOrderRemainingMs(first, now), 30_000)
  assert.equal(secondsOrderRemainingMs(settled, now), 0)
  assert.equal(secondsOrderProgress(first, now), 50)
  assert.ok(Math.abs(secondsOrderProgress(second, now) - 100 / 6) < 1e-12)
  assert.equal(secondsOrderProgress(pending, now), 0)
  assert.equal(secondsOrderEstimatedProfit(first), 8)
  assert.equal(secondsOrderEstimatedProfit(second), 18)
})

test('历史筛选复用活动状态判定并保留真实结果状态', () => {
  const order = (id: number, status: string, result?: string): SecondsOrder => ({
    id,
    symbol: 'BTCUSDT',
    stakeAssetSymbol: 'USDT',
    direction: 'up',
    stakeAmount: 10,
    durationSeconds: 60,
    payoutRate: .8,
    status,
    result,
    createdAt: 1_720_000_000_000 + id,
    expiresAt: 1_720_000_060_000 + id,
  })
  const orders = [
    order(1, 'opened'),
    order(2, 'PENDING'),
    order(3, 'active'),
    order(4, 'settled', 'win'),
    order(5, 'settled', 'loss'),
    order(6, 'cancelled'),
  ]

  assert.deepEqual(orders.filter((item) => !isActiveSecondsOrder(item)).map(({ id }) => id), [4, 5, 6])
  assert.deepEqual(historicalSecondsOrders([orders[5], orders[3], orders[0], orders[4]]).map(({ id }) => id), [6, 5, 4])
  assert.deepEqual(secondsOrderStatusPresentation(orders[3]), {
    translationKey: 'seconds.statusWon',
    source: 'win',
    tone: 'positive',
  })
  assert.deepEqual(secondsOrderStatusPresentation(orders[4]), {
    translationKey: 'seconds.statusLost',
    source: 'loss',
    tone: 'negative',
  })
  assert.deepEqual(secondsOrderStatusPresentation(order(7, 'future-state')), {
    translationKey: undefined,
    source: 'future-state',
    tone: 'pending',
  })
  assert.deepEqual(secondsOrderStatusPresentation(order(8, 'settled', 'future-result')), {
    translationKey: undefined,
    source: 'future-result',
    tone: 'pending',
  })
  assert.deepEqual(secondsOrderStatusPresentation(order(9, 'settled')), {
    translationKey: 'seconds.statusSettled',
    source: 'settled',
    tone: 'pending',
  })
})

test('秒合约订单适配器不会把无效 API 价格伪装为零价', () => {
  const order = mapSecondsOrder({
    id: 44,
    symbol: 'BTCUSDT',
    stake_asset_symbol: 'USDT',
    direction: 'up',
    stake_amount: '10',
    duration_seconds: 60,
    payout_rate: '.8',
    entry_price: 'not-a-price',
    settlement_price: false,
    status: 'settled',
    result: 'win',
    expires_at: 1_722_000_060_000,
    created_at: 1_722_000_000_000,
  })

  assert.equal(order.entryPrice, undefined)
  assert.equal(order.settlementPrice, undefined)
})

test('开仓返回值先按 ID 提交，迟到或缺失的列表刷新不会丢单', () => {
  const order = (id: number, status = 'opened'): SecondsOrder => ({
    id,
    symbol: id === 1 ? 'BTCUSDT' : 'ETHUSDT',
    stakeAssetSymbol: 'USDT',
    direction: id === 1 ? 'up' : 'down',
    stakeAmount: id * 10,
    durationSeconds: 60,
    payoutRate: .8,
    status,
    createdAt: 1_720_000_000_000 + id,
    expiresAt: 1_720_000_060_000 + id,
  })
  const first = order(1)
  const second = order(2)

  assert.deepEqual(upsertSecondsOrder([first], second).map(({ id }) => id), [2, 1])
  assert.deepEqual(upsertSecondsOrder([first, second], second).map(({ id }) => id), [2, 1])

  const staleRefresh = mergeSecondsOrderReconciliation([first], [second])
  assert.deepEqual(staleRefresh.map(({ id }) => id), [2, 1])
  assert.equal(staleRefresh[0], second)

  const settledSecond = order(2, 'settled')
  const authoritativeRefresh = mergeSecondsOrderReconciliation(
    [settledSecond, first],
    [second],
  )
  assert.deepEqual(authoritativeRefresh.map(({ id, status }) => ({ id, status })), [
    { id: 2, status: 'settled' },
    { id: 1, status: 'opened' },
  ])
})
