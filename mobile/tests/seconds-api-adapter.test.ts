import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import {
  activeSecondsOrders,
  createSecondsSettlementResultTracker,
  enqueueSecondsSettlementResults,
  historicalSecondsOrders,
  isActiveSecondsOrder,
  mapSecondsOrder,
  mergeSecondsOrderReconciliation,
  secondsOrderEstimatedProfit,
  secondsOrderProfitLossPresentation,
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
    stakeAmountText: '100',
    durationSeconds: 30,
    payoutRate: .924,
    payoutRateText: '0.924',
    entryPrice: 63080,
    entryPriceText: '63080',
    settlementPrice: undefined,
    settlementPriceText: null,
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
    ...orderFinancialFields(id * 10, id === 1 ? .8 : .9),
    durationSeconds: 60,
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
  assert.equal(secondsOrderEstimatedProfit(first), '8')
  assert.equal(secondsOrderEstimatedProfit(second), '18')
})

test('历史筛选复用活动状态判定并保留真实结果状态', () => {
  const order = (id: number, status: string, result?: string): SecondsOrder => ({
    id,
    symbol: 'BTCUSDT',
    stakeAssetSymbol: 'USDT',
    direction: 'up',
    ...orderFinancialFields(10, .8),
    durationSeconds: 60,
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

test('历史盈亏只使用订单快照，并区分净盈利、负本金和未知结果', () => {
  const order = (result?: string): SecondsOrder => ({
    id: 1,
    symbol: 'BTCUSDT',
    stakeAssetSymbol: 'USDT',
    direction: 'up',
    ...orderFinancialFields(100, .8),
    durationSeconds: 60,
    status: result ? 'settled' : 'cancelled',
    result,
    createdAt: 1_720_000_000_000,
    expiresAt: 1_720_000_060_000,
  })

  assert.deepEqual(secondsOrderProfitLossPresentation(order(' WIN ')), {
    translationKey: 'seconds.profitAmount',
    amountText: '80',
    amount: 80,
    tone: 'positive',
  })
  assert.deepEqual(secondsOrderProfitLossPresentation(order('loss')), {
    translationKey: 'seconds.lossAmount',
    amountText: '-100',
    amount: -100,
    tone: 'negative',
  })
  assert.deepEqual(secondsOrderProfitLossPresentation(order()), {
    translationKey: 'seconds.profitLossAmount',
    amountText: null,
    amount: undefined,
    tone: 'pending',
  })
  assert.deepEqual(secondsOrderProfitLossPresentation(order('future-result')), {
    translationKey: 'seconds.profitLossAmount',
    amountText: null,
    amount: undefined,
    tone: 'pending',
  })
})

test('页面会话追踪器不补弹历史结果，仅提示本页观察过的活动订单', () => {
  const tracker = createSecondsSettlementResultTracker()
  const active = settlementOrder(1, 'opened', undefined, 1_720_000_060_000)
  const historicalWin = settlementOrder(90, 'settled', 'win', 1_719_000_060_000)

  assert.deepEqual(tracker.reconcile([historicalWin, active]), [])

  assert.deepEqual(tracker.reconcile([{ ...active, result: 'win' }]), [])
  assert.equal(tracker.isTracking(active.id), true)
  assert.deepEqual(tracker.reconcile([{ ...active, status: 'expired', result: 'win' }]), [])
  assert.equal(tracker.isTracking(active.id), true)
  const settled = { ...active, status: 'settled', result: 'win' }
  assert.deepEqual(tracker.reconcile([settled]).map(({ id }) => id), [1])
  assert.deepEqual(tracker.reconcile([historicalWin, settled]), [])
})

test('结算追踪器保留暂缺结果的终态，并永不为取消订单生成输赢', () => {
  const tracker = createSecondsSettlementResultTracker()
  const delayed = settlementOrder(2, 'active', undefined, 1_720_000_120_000)
  const cancelled = settlementOrder(3, 'pending', undefined, 1_720_000_180_000)
  tracker.reconcile([delayed, cancelled])

  assert.deepEqual(tracker.reconcile([
    { ...delayed, status: 'settled' },
    { ...cancelled, status: 'cancelled' },
  ]), [])
  assert.equal(tracker.isTracking(delayed.id), true)
  assert.equal(tracker.isTracking(cancelled.id), false)
  assert.deepEqual(tracker.reconcile([{ ...cancelled, status: 'active' }]), [])
  assert.equal(tracker.isTracking(cancelled.id), false)
  assert.deepEqual(
    tracker.reconcile([{ ...delayed, status: 'settled', result: 'loss' }]).map(({ id }) => id),
    [2],
  )
  assert.equal(tracker.isTracking(delayed.id), false)
  assert.deepEqual(
    tracker.reconcile([{ ...cancelled, status: 'settled', result: 'win' }]),
    [],
  )
})

test('同批结算按到期时间和 ID 稳定排序，FIFO 队列按订单 ID 去重', () => {
  const tracker = createSecondsSettlementResultTracker()
  const first = settlementOrder(1, 'opened', undefined, 1_720_000_120_000)
  const second = settlementOrder(2, 'opened', undefined, 1_720_000_120_000)
  const third = settlementOrder(3, 'opened', undefined, 1_720_000_180_000)
  tracker.reconcile([third, second, first])

  const resolved = tracker.reconcile([
    { ...third, status: 'settled', result: 'loss' },
    { ...second, status: 'settled', result: 'win' },
    { ...first, status: 'settled', result: 'win' },
  ])
  assert.deepEqual(resolved.map(({ id }) => id), [1, 2, 3])
  assert.deepEqual(
    enqueueSecondsSettlementResults([resolved[0]], [resolved[0], resolved[1], resolved[1], resolved[2]])
      .map(({ id }) => id),
    [1, 2, 3],
  )
  assert.deepEqual(tracker.reconcile([...resolved].reverse()), [])
})

test('开仓响应可立即纳入会话追踪，重置后不会跨会话重放', () => {
  const tracker = createSecondsSettlementResultTracker()
  const opened = settlementOrder(7, 'opened', undefined, 1_720_000_240_000)
  tracker.track(opened)
  assert.deepEqual(
    tracker.reconcile([{ ...opened, status: 'settled', result: 'win' }]).map(({ id }) => id),
    [7],
  )

  tracker.reset()
  assert.deepEqual(
    tracker.reconcile([{ ...opened, status: 'settled', result: 'win' }]),
    [],
  )
})

test('秒合约订单适配器对无效或 JSON-number 金融权威字段 fail closed', () => {
  const base = {
    id: 44,
    symbol: 'BTCUSDT',
    stake_asset_symbol: 'USDT',
    direction: 'up',
    stake_amount: '10',
    duration_seconds: 60,
    payout_rate: '.8',
    entry_price: '63000',
    settlement_price: null,
    status: 'settled',
    result: 'win',
    expires_at: 1_722_000_060_000,
    created_at: 1_722_000_000_000,
  }

  assert.throws(() => mapSecondsOrder({ ...base, entry_price: 'not-a-price' }), /entry_price/)
  assert.throws(() => mapSecondsOrder({ ...base, settlement_price: false }), /settlement_price/)
  assert.throws(() => mapSecondsOrder({ ...base, stake_amount: 10 }), /stake_amount/)
  assert.throws(() => mapSecondsOrder({ ...base, payout_rate: 0.8 }), /payout_rate/)
  assert.throws(() => mapSecondsOrder({ ...base, entry_price: 1e-18 }), /entry_price/)
  assert.throws(() => mapSecondsOrder({ ...base, stake_amount: '1e-18' }), /stake_amount/)
})

test('秒合约订单以 DecimalText 精确保留超 2^53 与 1e-18 并计算盈亏', () => {
  const order = mapSecondsOrder({
    id: 45,
    symbol: 'BTCUSDT',
    stake_asset_symbol: 'USDT',
    direction: 'up',
    stake_amount: ' 9007199254740993.000000000000000001 ',
    duration_seconds: 60,
    payout_rate: '0.000000000000000001',
    entry_price: '9007199254740993.000000000000000001',
    settlement_price: '0.000000000000000001',
    status: 'settled',
    result: 'win',
    expires_at: 1_722_000_060_000,
    created_at: 1_722_000_000_000,
  })

  assert.equal(order.stakeAmountText, '9007199254740993.000000000000000001')
  assert.equal(order.payoutRateText, '0.000000000000000001')
  assert.equal(order.entryPriceText, '9007199254740993.000000000000000001')
  assert.equal(order.settlementPriceText, '0.000000000000000001')
  assert.equal(secondsOrderEstimatedProfit(order), '0.009007199254740993000000000000000001')
  assert.equal(
    secondsOrderProfitLossPresentation(order).amountText,
    '0.009007199254740993000000000000000001',
  )
  assert.equal(
    secondsOrderProfitLossPresentation({ ...order, result: 'loss' }).amountText,
    '-9007199254740993.000000000000000001',
  )
})

test('开仓返回值先按 ID 提交，迟到或缺失的列表刷新不会丢单', () => {
  const order = (id: number, status = 'opened'): SecondsOrder => ({
    id,
    symbol: id === 1 ? 'BTCUSDT' : 'ETHUSDT',
    stakeAssetSymbol: 'USDT',
    direction: id === 1 ? 'up' : 'down',
    ...orderFinancialFields(id * 10, .8),
    durationSeconds: 60,
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

function settlementOrder(
  id: number,
  status: string,
  result: string | undefined,
  expiresAt: number,
): SecondsOrder {
  return {
    id,
    symbol: id % 2 ? 'BTCUSDT' : 'ETHUSDT',
    stakeAssetSymbol: 'USDT',
    direction: id % 2 ? 'up' : 'down',
    ...orderFinancialFields(100, .8),
    durationSeconds: 60,
    status,
    result,
    createdAt: expiresAt - 60_000,
    expiresAt,
  }
}

function orderFinancialFields(
  stakeAmount: number,
  payoutRate: number,
): Pick<SecondsOrder,
  | 'stakeAmount'
  | 'stakeAmountText'
  | 'payoutRate'
  | 'payoutRateText'
  | 'entryPriceText'
  | 'settlementPriceText'
> {
  return {
    stakeAmount,
    stakeAmountText: normalizeDecimalText(String(stakeAmount)),
    payoutRate,
    payoutRateText: normalizeDecimalText(String(payoutRate)),
    entryPriceText: null,
    settlementPriceText: null,
  }
}
