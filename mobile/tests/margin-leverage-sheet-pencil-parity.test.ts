import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const sheetSource = read('../src/components/ContractTradeSheets.vue')
const tradeSource = read('../src/views/TradeView.vue')
const apiSource = read('../src/api/trading.ts')
const styleSource = sheetSource.match(/<style\s+scoped>([\s\S]*?)<\/style>/)?.[1] || ''

function message(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

test('调整杠杆弹窗声明当前 Pencil NTiiS/CulR4 双方向结构', () => {
  assert.match(sheetSource, /data-pencil-source="[^"]*NTiiS[^"]*CulR4[^"]*"/)
  assert.match(sheetSource, /class="contract-leverage-direction contract-leverage-direction--long"/)
  assert.match(sheetSource, /class="contract-leverage-direction contract-leverage-direction--short"/)
  assert.match(sheetSource, /draftLongLeverage/)
  assert.match(sheetSource, /draftShortLeverage/)
  assert.match(sheetSource, /contract-leverage-stepper/)
  assert.match(sheetSource, /contract-leverage-window/)
  assert.match(sheetSource, /contract-leverage-info/)
  assert.doesNotMatch(sheetSource, /contract-leverage-slider|contract-scope-row/)
})

test('Pencil 390x920 参考几何锁定 840px 面板、34px Header 与 52px 确认按钮', () => {
  assert.match(sheetSource, /\.contract-sheet--leverage\s*\{[^}]*height:\s*min\(840px,/s)
  assert.match(sheetSource, /\.contract-sheet--leverage\s*\{[^}]*border-radius:\s*24px 24px 0 0;/s)
  assert.match(sheetSource, /\.contract-sheet--leverage\s*\{[^}]*padding:\s*18px 20px calc\(16px \+ env\(safe-area-inset-bottom\)\);/s)
  assert.match(sheetSource, /\.contract-sheet--leverage \.contract-sheet__header\s*\{[^}]*height:\s*34px;/s)
  assert.match(sheetSource, /\.contract-leverage-stepper\s*\{[^}]*height:\s*64px;/s)
  assert.match(sheetSource, /\.contract-leverage-step\s*\{[^}]*height:\s*42px;[^}]*min-height:\s*42px;[^}]*width:\s*42px;/s)
  assert.match(sheetSource, /\.contract-leverage-value strong\s*\{[^}]*font-size:\s*52px;/s)
  assert.match(sheetSource, /\.contract-leverage-window\s*\{[^}]*height:\s*46px;[^}]*border-radius:\s*23px;/s)
  assert.match(sheetSource, /\.contract-sheet--leverage \.contract-sheet__submit\s*\{[^}]*height:\s*52px;[^}]*border-radius:\s*26px;/s)
  assert.match(sheetSource, /\.contract-leverage-info dt,[\s\S]*?line-height:\s*19px;/)
})

test('深色 scoped CSS 编译后保留 Teleport 面板后代选择器与 Pencil 色板', () => {
  const result = compileStyle({
    source: styleSource,
    filename: 'ContractTradeSheets.vue',
    id: 'data-v-contract-trade-sheets',
    scoped: true,
  })
  assert.deepEqual(result.errors, [])
  assert.match(result.code, /html\[data-theme=['"]dark['"]\]\s+\.contract-sheet--leverage\s*\{/)
  assert.match(result.code, /--leverage-sheet-page:\s*#0b0f0d;/i)
  assert.match(result.code, /--leverage-sheet-field:\s*#181e1a;/i)
  assert.match(result.code, /--leverage-sheet-text:\s*#f5f7f6;/i)
  assert.doesNotMatch(result.code, /html\[data-theme=['"]dark['"]\]\s*\{[^}]*--leverage-sheet-/)
})

test('移动端设置和下单链路使用做多与做空两个真实默认倍数', () => {
  assert.match(apiSource, /longLeverage:\s*number \| null/)
  assert.match(apiSource, /shortLeverage:\s*number \| null/)
  assert.match(apiSource, /long_leverage:/)
  assert.match(apiSource, /short_leverage:/)
  assert.match(tradeSource, /const longLeverage = ref\(/)
  assert.match(tradeSource, /const shortLeverage = ref\(/)
  assert.match(tradeSource, /side\.value === 'buy'\s*\?\s*longLeverage\.value\s*:\s*shortLeverage\.value/)
  assert.match(tradeSource, /await updateMarginLeverage\(product\.id,\s*\{[\s\S]*?longLeverage:[\s\S]*?shortLeverage:/)
  assert.match(tradeSource, /const requestVersion = \+\+marginSettingRequestVersion[\s\S]*?await updateMarginLeverage/)
  assert.match(tradeSource, /requestVersion !== marginSettingRequestVersion[\s\S]*?session\.token !== requestSessionKey[\s\S]*?selectedProduct\.value\?\.id !== product\.id/)
  assert.match(tradeSource, /longActionCompact', \{ leverage: longLeverage \}/)
  assert.match(tradeSource, /shortActionCompact', \{ leverage: shortLeverage \}/)
})

test('新增杠杆弹窗文案保持中英文对称且模板无固定中文', () => {
  const keys = [
    'trade.longLeverage',
    'trade.shortLeverage',
    'trade.leverageMaxOpenAfterAdjust',
    'trade.requiredMarginAfterAdjust',
    'trade.moreLeverageOptions',
    'trade.leverageFutureOrdersOnly',
  ]
  for (const key of keys) {
    assert.notEqual(message(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(message(en, key), undefined, `en missing ${key}`)
  }
  const leverageTemplate = sheetSource.match(/<template v-else-if="open === 'leverage'">([\s\S]*?)<template v-else-if="open === 'marginMode'">/)?.[1] || ''
  assert.doesNotMatch(leverageTemplate, /[\u3400-\u9fff]|\p{Extended_Pictographic}/u)
})
