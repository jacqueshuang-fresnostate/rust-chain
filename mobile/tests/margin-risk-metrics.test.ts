import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  estimateIsolatedLiquidationPrice,
  mapMarginCrossAccountRisk,
  parseMarginRiskNumber,
  resolveMaintenanceMarginRate,
  resolveMarginPositionRiskMetrics,
} from '../src/core/marginRiskMetrics.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const isolatedPosition = {
  marginMode: 'isolated',
  direction: 'long',
  entryPrice: 100,
  notionalAmount: 100,
  marginAmount: 20,
  interestAmount: 1.5,
  productMaintenanceMarginRate: 0.05,
}

const tradeSource = read('../src/views/TradeView.vue')
const tradingApiSource = read('../src/api/trading.ts')
const marginRiskSource = read('../src/core/marginRiskMetrics.ts')
const typesSource = read('../src/core/types.ts')

const backendCrossAccountRisk = {
  margin_asset: 3,
  reference_pair_id: 7,
  price_assumption: 'reference_pair_only_other_marks_static',
  equity: '57.000000000000000000',
  maintenance_margin: '12.000000000000000000',
  liquidation_buffer: '45.000000000000000000',
  margin_ratio: '4.750000000000000000',
  unrealized_pnl: '-1.500000000000000000',
  interest_amount: '3.000000000000000000',
  should_liquidate: false,
  net_quantity: '0.600000000000000000',
  gross_quantity: '1.400000000000000000',
  estimate_status: 'estimated',
  conditional_liquidation_price: '25.000000000000000000',
  conditional_liquidation_distance_rate: '0.750000000000000000',
  marks_observed_at_min: 1_787_200_000_000,
  marks_observed_at_max: 1_787_200_001_000,
}

test('风险 DTO 数值解析限定在有限 JSON 数字与十进制字符串', () => {
  assert.equal(parseMarginRiskNumber(0.05), 0.05)
  assert.equal(parseMarginRiskNumber(' 0.0500 '), 0.05)
  assert.equal(parseMarginRiskNumber('.5'), 0.5)
  assert.equal(parseMarginRiskNumber('-0'), 0)
  assert.equal(Object.is(parseMarginRiskNumber('-0'), -0), false)

  for (const value of [undefined, null, '', '1e-3', '0x10', true, [], {}, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(parseMarginRiskNumber(value), null)
  }
})

test('全仓账户风险对象逐字段严格映射且保留可空估算值', () => {
  assert.deepEqual(mapMarginCrossAccountRisk(backendCrossAccountRisk), {
    marginAssetId: 3,
    referencePairId: 7,
    priceAssumption: 'reference_pair_only_other_marks_static',
    equity: 57,
    maintenanceMargin: 12,
    liquidationBuffer: 45,
    marginRatio: 4.75,
    unrealizedPnl: -1.5,
    interestAmount: 3,
    shouldLiquidate: false,
    netQuantity: 0.6,
    grossQuantity: 1.4,
    estimateStatus: 'estimated',
    conditionalLiquidationPrice: 25,
    conditionalLiquidationDistanceRate: 0.75,
    marksObservedAtMin: 1_787_200_000_000,
    marksObservedAtMax: 1_787_200_001_000,
  })
  assert.equal(mapMarginCrossAccountRisk(undefined), undefined)
  assert.equal(mapMarginCrossAccountRisk(null), undefined)

  const unavailable = mapMarginCrossAccountRisk({
    ...backendCrossAccountRisk,
    margin_ratio: null,
    estimate_status: 'net_delta_zero',
    conditional_liquidation_price: null,
    conditional_liquidation_distance_rate: null,
  })
  assert.equal(unavailable?.marginRatio, null)
  assert.equal(unavailable?.conditionalLiquidationPrice, null)
  assert.equal(unavailable?.conditionalLiquidationDistanceRate, null)
})

test('全仓账户风险对象拒绝缺字段、非有限数、错误类型与反向时间范围', () => {
  const malformedValues: Array<[string, unknown]> = [
    ['margin_asset', 0],
    ['reference_pair_id', Number.MAX_SAFE_INTEGER + 1],
    ['price_assumption', 'single_position'],
    ['equity', '1e3'],
    ['maintenance_margin', Number.NaN],
    ['liquidation_buffer', undefined],
    ['margin_ratio', 'Infinity'],
    ['unrealized_pnl', {}],
    ['interest_amount', ''],
    ['should_liquidate', 'false'],
    ['net_quantity', []],
    ['gross_quantity', true],
    ['estimate_status', '   '],
    ['conditional_liquidation_price', undefined],
    ['conditional_liquidation_distance_rate', 'NaN'],
    ['marks_observed_at_min', null],
    ['marks_observed_at_max', 1.5],
  ]
  for (const [field, value] of malformedValues) {
    assert.throws(
      () => mapMarginCrossAccountRisk({ ...backendCrossAccountRisk, [field]: value }),
      /invalid cross account risk/,
      field,
    )
  }
  assert.throws(
    () => mapMarginCrossAccountRisk({
      ...backendCrossAccountRisk,
      marks_observed_at_min: backendCrossAccountRisk.marks_observed_at_max + 1,
    }),
    /mark observation range/,
  )
  assert.throws(() => mapMarginCrossAccountRisk([]), /invalid cross account risk object/)
})

test('维持保证金率优先服务端风险快照，无效时回退产品配置', () => {
  assert.equal(resolveMaintenanceMarginRate(0.08, 0.05), 0.08)
  assert.equal(resolveMaintenanceMarginRate(0, 0.05), 0)
  assert.equal(resolveMaintenanceMarginRate(null, 0.05), 0.05)
  assert.equal(resolveMaintenanceMarginRate(Number.NaN, 0.05), 0.05)
  assert.equal(resolveMaintenanceMarginRate(-0.01, 0.05), 0.05)
  assert.equal(resolveMaintenanceMarginRate(undefined, Number.POSITIVE_INFINITY), null)
  assert.equal(resolveMaintenanceMarginRate(undefined, -0.01), null)
})

test('有效服务端预估强平价直接优先于本地公式', () => {
  const metrics = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    entryPrice: null,
    serverMaintenanceMarginRate: 0.06,
    serverEstimatedLiquidationPrice: 91.25,
  })

  assert.equal(metrics.maintenanceMarginRate, 0.06)
  assert.equal(metrics.estimatedLiquidationPrice, 91.25)
  assert.equal(metrics.liquidationDistanceRate, null)
  assert.equal(metrics.liquidationRiskScope, 'position')
  assert.equal(metrics.crossAccountEstimateState, null)
})

test('无效服务端预估强平价不会阻断逐仓同源公式回退', () => {
  for (const serverEstimatedLiquidationPrice of [undefined, null, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    const metrics = resolveMarginPositionRiskMetrics({
      ...isolatedPosition,
      serverEstimatedLiquidationPrice,
    })
    assert.equal(metrics.estimatedLiquidationPrice, 86.5)
  }
})

test('逐仓多仓与空仓在快照价缺失时使用后端同源公式', () => {
  const longPrice = estimateIsolatedLiquidationPrice({
    direction: 'long',
    entryPrice: 100,
    notionalAmount: 100,
    marginAmount: 20,
    interestAmount: 1.5,
    maintenanceMarginRate: 0.05,
  })
  const shortPrice = estimateIsolatedLiquidationPrice({
    direction: 'short',
    entryPrice: 100,
    notionalAmount: 100,
    marginAmount: 20,
    interestAmount: 1.5,
    maintenanceMarginRate: 0.05,
  })

  assert.equal(longPrice, 86.5)
  assert.equal(shortPrice, 113.5)

  const productFallback = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    serverMaintenanceMarginRate: null,
    serverEstimatedLiquidationPrice: null,
  })
  assert.equal(productFallback.maintenanceMarginRate, 0.05)
  assert.equal(productFallback.estimatedLiquidationPrice, 86.5)
})

test('全仓 estimated 快照使用账户条件强平价与距离', () => {
  const metrics = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    marginMode: 'cross',
    serverMaintenanceMarginRate: 0.06,
    serverEstimatedLiquidationPrice: 88,
    serverLiquidationDistanceRate: 0.99,
    crossAccountRisk: {
      estimateStatus: 'estimated',
      conditionalLiquidationPrice: 78.25,
      conditionalLiquidationDistanceRate: 0.2175,
    },
  })

  assert.equal(metrics.maintenanceMarginRate, 0.06)
  assert.equal(metrics.estimatedLiquidationPrice, 78.25)
  assert.equal(metrics.liquidationDistanceRate, 0.2175)
  assert.equal(metrics.liquidationRiskScope, 'account')
  assert.equal(metrics.crossAccountEstimateState, 'estimated')
})

test('完全/近似对冲、已触发与无正边界不会落入逐仓公式', () => {
  for (const estimateStatus of [
    'net_delta_zero',
    'net_delta_near_zero',
    'already_liquidatable',
    'no_positive_boundary',
    'mark_unavailable',
  ]) {
    const metrics = resolveMarginPositionRiskMetrics({
      ...isolatedPosition,
      marginMode: 'cross',
      serverEstimatedLiquidationPrice: 88,
      serverLiquidationDistanceRate: 0.12,
      crossAccountRisk: {
        estimateStatus,
        conditionalLiquidationPrice: 77,
        conditionalLiquidationDistanceRate: 0.23,
      },
    })

    assert.equal(metrics.estimatedLiquidationPrice, null, estimateStatus)
    assert.equal(metrics.liquidationDistanceRate, null, estimateStatus)
    assert.equal(metrics.liquidationRiskScope, 'account', estimateStatus)
    assert.equal(metrics.crossAccountEstimateState, 'unavailable', estimateStatus)
  }
})

test('旧后端缺少账户快照时保留账户级兼容文案', () => {
  const metrics = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    marginMode: 'cross',
    serverEstimatedLiquidationPrice: 88,
    serverLiquidationDistanceRate: 0.12,
  })

  assert.equal(metrics.estimatedLiquidationPrice, null)
  assert.equal(metrics.liquidationDistanceRate, null)
  assert.equal(metrics.liquidationRiskScope, 'account')
  assert.equal(metrics.crossAccountEstimateState, 'legacy')
})

test('全仓条件价非正或非有限时显式不可用，不伪造距离', () => {
  for (const conditionalLiquidationPrice of [undefined, null, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    const metrics = resolveMarginPositionRiskMetrics({
      ...isolatedPosition,
      marginMode: 'cross',
      crossAccountRisk: {
        estimateStatus: 'estimated',
        conditionalLiquidationPrice,
        conditionalLiquidationDistanceRate: 0.2,
      },
    })
    assert.equal(metrics.estimatedLiquidationPrice, null)
    assert.equal(metrics.liquidationDistanceRate, null)
    assert.equal(metrics.crossAccountEstimateState, 'unavailable')
  }

  const invalidDistance = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    marginMode: 'cross',
    crossAccountRisk: {
      estimateStatus: 'estimated',
      conditionalLiquidationPrice: 80,
      conditionalLiquidationDistanceRate: Number.NaN,
    },
  })
  assert.equal(invalidDistance.estimatedLiquidationPrice, 80)
  assert.equal(invalidDistance.liquidationDistanceRate, null)
  assert.equal(invalidDistance.crossAccountEstimateState, 'estimated')
})

test('逐仓公式严格拒绝缺失、非有限、越界与非正结果', () => {
  const valid = {
    direction: 'long',
    entryPrice: 100,
    notionalAmount: 100,
    marginAmount: 20,
    interestAmount: 0,
    maintenanceMarginRate: 0,
  }

  for (const entryPrice of [undefined, null, 0, -1, Number.NaN, Number.POSITIVE_INFINITY, '100']) {
    assert.equal(estimateIsolatedLiquidationPrice({ ...valid, entryPrice }), null)
  }
  for (const notionalAmount of [undefined, null, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(estimateIsolatedLiquidationPrice({ ...valid, notionalAmount }), null)
  }
  for (const marginAmount of [undefined, null, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(estimateIsolatedLiquidationPrice({ ...valid, marginAmount }), null)
  }
  for (const interestAmount of [undefined, null, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(estimateIsolatedLiquidationPrice({ ...valid, interestAmount }), null)
  }
  for (const maintenanceMarginRate of [undefined, null, -0.01, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(estimateIsolatedLiquidationPrice({ ...valid, maintenanceMarginRate }), null)
  }

  assert.equal(estimateIsolatedLiquidationPrice({ ...valid, direction: 'sideways' }), null)
  assert.equal(estimateIsolatedLiquidationPrice({
    ...valid,
    direction: 'short',
    marginAmount: 1,
    maintenanceMarginRate: 2,
  }), null)
  assert.equal(estimateIsolatedLiquidationPrice({
    ...valid,
    entryPrice: Number.MAX_VALUE,
    marginAmount: 1,
    interestAmount: 100,
  }), null)
})

test('持仓卡片接入纯函数并保留风险快照的部分成功合并', () => {
  const riskLoader = sliceBetween(
    tradeSource,
    'async function loadMarginPositionRisks',
    'function setQuantity',
  )

  assert.match(tradeSource, /import \{[\s\S]*?resolveMarginPositionRiskMetrics,[\s\S]*?\} from '@\/core\/marginRiskMetrics'/)
  assert.match(tradeSource, /serverMaintenanceMarginRate: risk\?\.maintenanceMarginRate/)
  assert.match(tradeSource, /productMaintenanceMarginRate: productForPosition\(position\)\?\.maintenanceMarginRate/)
  assert.match(tradeSource, /serverEstimatedLiquidationPrice: risk\?\.estimatedLiquidationPrice/)
  assert.match(tradeSource, /serverLiquidationDistanceRate: risk\?\.liquidationDistanceRate/)
  assert.match(tradeSource, /crossAccountRisk: risk\?\.crossAccountRisk/)
  assert.match(tradeSource, /const marginRiskDisplayMetricsByPositionId = computed/)
  assert.equal(
    tradeSource.match(/resolveMarginPositionRiskMetrics\(\{/g)?.length,
    1,
    'each position risk formula must be resolved once in the computed projection',
  )
  assert.match(tradeSource, /metrics\.crossAccountEstimateState === 'legacy'[\s\S]*?t\('trade\.crossAccountRisk'\)/)
  assert.match(tradeSource, /metrics\.crossAccountEstimateState === 'estimated'[\s\S]*?formatPrice\(metrics\.estimatedLiquidationPrice\)/)
  assert.match(tradeSource, /t\('trade\.noStableSingleLiquidationPrice'\)/)
  assert.match(tradeSource, /t\('trade\.estimatedAccountLiquidationPrice'\)/)
  assert.match(tradeSource, /const distance = positionRiskDisplayMetrics\(position\)\.liquidationDistanceRate/)
  assert.match(tradeSource, /formatRate\(positionRiskDisplayMetrics\(position\)\.liquidationDistanceRate\)/)
  assert.match(tradeSource, /role="note"[\s\S]*?t\('trade\.crossAccountLiquidationAssumption'\)/)
  assert.match(tradeSource, /formatRate\(positionRiskDisplayMetrics\(position\)\.maintenanceMarginRate\)/)
  assert.doesNotMatch(tradeSource, /positionMarginRatio|maintenanceMarginRatio|marginCrossAccounts|MarginCrossAccount/)
  assert.doesNotMatch(tradeSource, /\.marginRatio\b/)

  assert.match(riskLoader, /Promise\.allSettled\(eligible\.map/)
  assert.match(riskLoader, /Object\.entries\(marginRiskSnapshots\.value\)\.filter/)
  assert.match(riskLoader, /if \(result\.status === 'fulfilled'\) next\[result\.value\.positionId\] = result\.value/)
  assert.doesNotMatch(riskLoader, /marginPositions\.value\s*=/)

  assert.match(tradingApiSource, /maintenanceMarginRate: parseMarginRiskNumber\(product\.maintenance_margin_rate\)/)
  assert.match(tradingApiSource, /maintenanceMarginRate: parseMarginRiskNumber\(risk\.maintenance_margin_rate\)/)
  assert.match(tradingApiSource, /estimatedLiquidationPrice: parseMarginRiskNumber\(risk\.estimated_liquidation_price\)/)
  assert.match(tradingApiSource, /crossAccountRisk: mapMarginCrossAccountRisk\(risk\.cross_account_risk\)/)
  assert.match(tradingApiSource, /mapMarginCrossAccountRisk,[\s\S]*?type MarginCrossAccountRisk,[\s\S]*?from '@\/core\/marginRiskMetrics'/)
  assert.match(marginRiskSource, /export interface MarginCrossAccountRisk \{[\s\S]*?marginAssetId: number[\s\S]*?referencePairId: number[\s\S]*?priceAssumption: MarginCrossAccountPriceAssumption[\s\S]*?conditionalLiquidationPrice: number \| null[\s\S]*?conditionalLiquidationDistanceRate: number \| null[\s\S]*?marksObservedAtMin: number[\s\S]*?marksObservedAtMax: number/)
  for (const field of [
    'margin_asset',
    'reference_pair_id',
    'price_assumption',
    'equity',
    'maintenance_margin',
    'liquidation_buffer',
    'margin_ratio',
    'unrealized_pnl',
    'interest_amount',
    'should_liquidate',
    'net_quantity',
    'gross_quantity',
    'estimate_status',
    'conditional_liquidation_price',
    'conditional_liquidation_distance_rate',
    'marks_observed_at_min',
    'marks_observed_at_max',
  ]) {
    assert.match(marginRiskSource, new RegExp(`value\\.${field}`), field)
  }
  assert.match(marginRiskSource, /function strictMarginRiskNumber[\s\S]*?parseMarginRiskNumber\(value\)[\s\S]*?throw new TypeError/)
  assert.match(marginRiskSource, /priceAssumption !== 'reference_pair_only_other_marks_static'[\s\S]*?throw new TypeError/)
  const marginSubmitter = sliceBetween(
    tradingApiSource,
    'export async function placeMarginOrder',
    'export function createMarginOrderIdempotencyKey',
  )
  assert.doesNotMatch(marginSubmitter, /crossAccountRisk|conditionalLiquidation|liquidationBuffer/)
  assert.match(tradingApiSource, /marginAmount: requiredMarginRiskNumber\(position\.margin_amount\)/)
  assert.match(tradingApiSource, /notionalAmount: requiredMarginRiskNumber\(position\.notional_amount\)/)
  assert.match(tradingApiSource, /entryPrice: parseMarginRiskNumber\(position\.entry_price\)/)
  assert.match(tradingApiSource, /interestAmount: requiredMarginRiskNumber\(position\.interest_amount\)/)
  assert.match(tradingApiSource, /returnRate: optionalNumber\(risk\.return_rate\)/)
  assert.match(tradingApiSource, /marginRatio: optionalNumber\(risk\.margin_ratio\)/)
  const existingOptionalNumber = sliceBetween(
    tradingApiSource,
    'function optionalNumber',
    'function requiredMarginRiskNumber',
  )
  assert.match(existingOptionalNumber, /asNumber\(value, Number\.NaN\)/)
  assert.doesNotMatch(existingOptionalNumber, /parseMarginRiskNumber|MARGIN_RISK_DECIMAL_PATTERN/)
  assert.match(tradingApiSource, /maintenanceMarginRate: number \| null/)
  assert.match(typesSource, /maintenanceMarginRate: number \| null/)
})

test('风险指标中英文案对称且口径明确', () => {
  assert.equal(zhCN.trade.maintenanceMarginRate, '维持保证金率')
  assert.equal(en.trade.maintenanceMarginRate, 'Maintenance margin rate')
  assert.equal(zhCN.trade.crossAccountRisk, '账户级风控')
  assert.equal(en.trade.crossAccountRisk, 'Account-level risk')
  assert.equal(zhCN.trade.estimatedAccountLiquidationPrice, '账户预估强平价')
  assert.equal(en.trade.estimatedAccountLiquidationPrice, 'Est. account liquidation')
  assert.equal(zhCN.trade.noStableSingleLiquidationPrice, '无稳定单一价格')
  assert.equal(en.trade.noStableSingleLiquidationPrice, 'No stable single price')
  assert.equal(zhCN.trade.crossAccountLiquidationDistance, '账户条件距离')
  assert.equal(en.trade.crossAccountLiquidationDistance, 'Conditional account distance')
  assert.equal(zhCN.trade.crossAccountLiquidationAssumption, '仅当前交易对价格变动，其他行情保持不变。')
  assert.equal(en.trade.crossAccountLiquidationAssumption, 'Only this pair moves; other market prices stay unchanged.')
  assert.equal('maintenanceMarginRatio' in zhCN.trade, false)
  assert.equal('maintenanceMarginRatio' in en.trade, false)
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function sliceBetween(source: string, startToken: string, endToken: string): string {
  const start = source.indexOf(startToken)
  assert.notEqual(start, -1, `missing start token: ${startToken}`)
  const end = source.indexOf(endToken, start + startToken.length)
  assert.notEqual(end, -1, `missing end token: ${endToken}`)
  return source.slice(start, end)
}
