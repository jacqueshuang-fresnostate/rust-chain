import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createMarginLeveragePreview,
  mapMarginUserLeverageSetting,
  marginLeverageWindow,
  marginLeverageWindowStart,
  nextMarginLeverageWindowStart,
  normalizeMarginLeverageLevels,
  stepMarginLeverage,
} from '../src/core/marginLeverage.ts'

const levels = [1, 2, 3, 5, 10, 20, 30, 50, 75]

test('旧设置响应仅在缺少方向字段时回落 legacy 值', () => {
  assert.deepEqual(mapMarginUserLeverageSetting({ leverage: '5.00000000' }), {
    leverage: 5,
    longLeverage: 5,
    shortLeverage: 5,
  })
  assert.deepEqual(mapMarginUserLeverageSetting({
    leverage: '30.00000000',
    long_leverage: '30.00000000',
    short_leverage: '3.00000000',
  }), {
    leverage: 30,
    longLeverage: 30,
    shortLeverage: 3,
  })
})

test('新设置响应中显式非法方向值不会被 legacy 静默替代', () => {
  assert.deepEqual(mapMarginUserLeverageSetting({
    leverage: '5.00000000',
    long_leverage: null,
    short_leverage: '-2',
  }), {
    leverage: 5,
    longLeverage: null,
    shortLeverage: null,
  })
})

test('杠杆档位只保留后台正数唯一值并按升序排列', () => {
  assert.deepEqual(normalizeMarginLeverageLevels([5, 2, 5, 0, Number.NaN, 10]), [2, 5, 10])
})

test('加减只移动到相邻真实档位且不会越界', () => {
  assert.equal(stepMarginLeverage(levels, 3, 1), 5)
  assert.equal(stepMarginLeverage(levels, 3, -1), 2)
  assert.equal(stepMarginLeverage(levels, 1, -1), 1)
  assert.equal(stepMarginLeverage(levels, 75, 1), 75)
})

test('六档窗口复现 Pencil 的低倍与高倍快捷轨并可循环查看更多', () => {
  const shortStart = marginLeverageWindowStart(levels, 3)
  const longStart = marginLeverageWindowStart(levels, 30)
  assert.deepEqual(marginLeverageWindow(levels, shortStart), [1, 2, 3, 5, 10, 20])
  assert.deepEqual(marginLeverageWindow(levels, longStart), [5, 10, 20, 30, 50, 75])
  assert.equal(nextMarginLeverageWindowStart(levels, 0), 3)
  assert.equal(nextMarginLeverageWindowStart(levels, 3), 0)
})

test('预览用真实余额计算最大可开且只为逐仓给出同源强平估算', () => {
  const isolated = createMarginLeveragePreview({
    availableBalance: 1_000,
    referencePrice: 50_000,
    marginAmount: 100,
    leverage: 10,
    maintenanceMarginRate: 0.01,
    marginMode: 'isolated',
    direction: 'long',
  })
  assert.equal(isolated.maximumOpenQuantity, 0.2)
  assert.equal(isolated.requiredMargin, 100)
  assert.equal(isolated.estimatedLiquidationPrice, 45_500)

  const cross = createMarginLeveragePreview({
    availableBalance: 1_000,
    referencePrice: 50_000,
    marginAmount: 0,
    leverage: 10,
    maintenanceMarginRate: 0.01,
    marginMode: 'cross',
    direction: 'short',
  })
  assert.equal(cross.maximumOpenQuantity, 0.2)
  assert.equal(cross.requiredMargin, 0)
  assert.equal(cross.estimatedLiquidationPrice, null)
})
