import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  estimateIsolatedLiquidationPrice,
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
const typesSource = read('../src/core/types.ts')

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
  assert.equal(metrics.liquidationRiskScope, 'position')
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

test('全仓始终保留账户级风控语义，不接受或推导单仓强平价', () => {
  const metrics = resolveMarginPositionRiskMetrics({
    ...isolatedPosition,
    marginMode: 'cross',
    serverMaintenanceMarginRate: 0.06,
    serverEstimatedLiquidationPrice: 88,
  })

  assert.equal(metrics.maintenanceMarginRate, 0.06)
  assert.equal(metrics.estimatedLiquidationPrice, null)
  assert.equal(metrics.liquidationRiskScope, 'account')
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
  assert.match(tradeSource, /const marginRiskDisplayMetricsByPositionId = computed/)
  assert.equal(
    tradeSource.match(/resolveMarginPositionRiskMetrics\(\{/g)?.length,
    1,
    'each position risk formula must be resolved once in the computed projection',
  )
  assert.match(tradeSource, /metrics\.liquidationRiskScope === 'account'[\s\S]*?t\('trade\.crossAccountRisk'\)/)
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
