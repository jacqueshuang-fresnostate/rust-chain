import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import {
  SLIDE_CONFIRM_THRESHOLD,
  isSlideConfirmComplete,
  slideProgressForKey,
  slideProgressFromClientX,
} from '../src/core/slideToConfirm.ts'
import {
  marginClosePreviewAmount,
  normalizeMarginClosePercentage,
} from '../src/core/marginClose.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const tradeSource = read('../src/views/TradeView.vue')
const sheetSource = read('../src/components/MarginCloseSheet.vue')
const sheetStyle = sheetSource.match(/<style\s+scoped>([\s\S]*?)<\/style>/)?.[1] || ''
const tradingApiSource = read('../src/api/trading.ts')

test('部分平仓比例限制在 1..100 且预览金额按冻结比例派生', () => {
  assert.equal(normalizeMarginClosePercentage(-5), 1)
  assert.equal(normalizeMarginClosePercentage(37.4), 37)
  assert.equal(normalizeMarginClosePercentage(1000), 100)
  assert.equal(marginClosePreviewAmount(0.6369, 37), 0.235653)
  assert.equal(marginClosePreviewAmount(-12.27, 25), -3.0675)
  assert.equal(marginClosePreviewAmount(null, 50), null)
})

test('拖动进度在手柄可移动区间内归一化并使用明确确认阈值', () => {
  assert.equal(slideProgressFromClientX(31, 0, 350, 50, 6), 0)
  assert.equal(slideProgressFromClientX(319, 0, 350, 50, 6), 1)
  assert.equal(slideProgressFromClientX(-100, 0, 350, 50, 6), 0)
  assert.equal(slideProgressFromClientX(900, 0, 350, 50, 6), 1)
  assert.equal(slideProgressFromClientX(175, 0, 350, 50, 6), 0.5)
  assert.equal(SLIDE_CONFIRM_THRESHOLD, 0.9)
  assert.equal(isSlideConfirmComplete(0.899), false)
  assert.equal(isSlideConfirmComplete(0.9), true)
})

test('键盘可连续推进或复位拖动进度且不会用无关按键改变状态', () => {
  assert.equal(slideProgressForKey(0.4, 'ArrowRight'), 0.5)
  assert.equal(slideProgressForKey(0.4, 'ArrowUp'), 0.5)
  assert.equal(slideProgressForKey(0.4, 'ArrowLeft'), 0.3)
  assert.equal(slideProgressForKey(0.4, 'ArrowDown'), 0.3)
  assert.equal(slideProgressForKey(0.4, 'Home'), 0)
  assert.equal(slideProgressForKey(0.4, 'End'), 1)
  assert.equal(slideProgressForKey(0.4, 'Tab'), null)
})

test('平仓底部弹窗绑定当前 Pencil 浅色与深色选稿并保留关键几何', () => {
  assert.match(sheetSource, /data-pencil-source="ajSJF DGiNR"/)
  assert.match(sheetSource, /class="margin-close-sheet"/)
  assert.match(sheetSource, /\.margin-close-sheet \{[\s\S]*?border-radius: 24px 24px 0 0;[\s\S]*?grid-template-rows:[\s\S]*?40px[\s\S]*?38px[\s\S]*?58px[\s\S]*?17px[\s\S]*?58px[\s\S]*?24px[\s\S]*?69px[\s\S]*?minmax\(0, 32px\)[\s\S]*?62px;[\s\S]*?padding: 14px 20px calc\(16px \+ env\(safe-area-inset-bottom, 0px\)\);/)
  assert.match(sheetSource, /--close-sheet-page: #ffffff;/)
  assert.match(sheetSource, /--close-sheet-field: #f2f4f3;/)
  assert.match(sheetSource, /--close-sheet-text: #101512;/)
  assert.match(sheetSource, /--close-sheet-action: #ff3e73;/)
  assert.match(sheetSource, /html\[data-theme='dark'\][\s\S]*?--close-sheet-page: #0b0f0d;[\s\S]*?--close-sheet-field: #181e1a;[\s\S]*?--close-sheet-text: #f5f7f6;/)
})

test('scoped CSS 编译保留 html 深色主题根与 Teleport 平仓弹窗的后代关系', () => {
  assert.ok(sheetStyle, 'MarginCloseSheet.vue 必须保留可编译的 scoped style')
  const compiled = compileStyle({
    source: sheetStyle,
    filename: 'MarginCloseSheet.vue',
    id: 'data-v-margin-close-sheet',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])

  const compiledCss = compiled.code.replace(/\/\*[\s\S]*?\*\//g, '')
  const darkDescendantRule = compiledCss.match(
    /html\[data-theme=['"]dark['"]\]\s+\.margin-close-sheet\s*\{([^}]*)\}/,
  )
  assert.ok(darkDescendantRule, '编译结果必须保留 html[data-theme="dark"] .margin-close-sheet 选择器')
  for (const [token, value] of [
    ['page', '#0b0f0d'],
    ['field', '#181e1a'],
    ['text', '#f5f7f6'],
    ['line', '#303a35'],
  ] as const) {
    assert.match(
      darkDescendantRule[1],
      new RegExp(`--close-sheet-${token}:\\s*${value};`),
      `深色后代规则必须声明 --close-sheet-${token}: ${value}`,
    )
  }
  assert.doesNotMatch(
    compiledCss,
    /html\[data-theme=['"]dark['"]\]\s*\{[^}]*--close-sheet-[\w-]+\s*:/,
    '任何平仓弹窗深色变量均不得退化到裸 html[data-theme="dark"] 规则',
  )
})

test('弹窗只呈现真实仓位数据并用可拖动比例派生部分平仓预览', () => {
  assert.match(sheetSource, /props\.symbol/)
  assert.match(sheetSource, /props\.markPrice/)
  assert.match(sheetSource, /props\.positionQuantity/)
  assert.match(sheetSource, /props\.estimatedPnl/)
  assert.match(sheetSource, /type="range"/)
  assert.match(sheetSource, /class="margin-close-sheet__ratio-input"/)
  assert.match(sheetSource, /min="1"/)
  assert.match(sheetSource, /max="100"/)
  assert.match(sheetSource, /step="1"/)
  assert.match(sheetSource, /v-model\.number="closePercentage"/)
  assert.match(sheetSource, /t\('trade\.marginCloseMarketPrice'\)/)
  assert.match(sheetSource, /t\('trade\.marginClosePercentage', \{ percentage: closePercentage \}\)/)
  assert.match(sheetSource, /marginClosePreviewAmount\(props\.positionQuantity, closePercentage\.value\)/)
  assert.match(sheetSource, /marginClosePreviewAmount\(props\.estimatedPnl, closePercentage\.value\)/)
  assert.doesNotMatch(sheetSource, /79,800|0\.6369|12\.27|BTCUSDT/)
})

test('平仓确认轨道覆盖指针、键盘、ARIA、请求锁和未达阈值回弹', () => {
  assert.match(sheetSource, /role="slider"/)
  assert.match(sheetSource, /aria-valuemin="0"/)
  assert.match(sheetSource, /aria-valuemax="100"/)
  assert.match(sheetSource, /:aria-valuenow="Math\.round\(slideProgress \* 100\)"/)
  assert.match(sheetSource, /@pointerdown="handleSlidePointerDown"/)
  assert.match(sheetSource, /@pointermove="handleSlidePointerMove"/)
  assert.match(sheetSource, /@pointerup="handleSlidePointerUp"/)
  assert.match(sheetSource, /@pointercancel="handleSlidePointerCancel"/)
  assert.match(sheetSource, /event\.target instanceof Element[\s\S]*?closest\('\.margin-close-slide__handle'\)/)
  assert.match(sheetSource, /setPointerCapture\(event\.pointerId\)/)
  assert.match(sheetSource, /if \(isSlideConfirmComplete\(slideProgress\.value\)\)[\s\S]*?requestConfirm\(\)[\s\S]*?resetSlide\(\)/)
  assert.match(sheetSource, /if \(event\.key === 'Enter' \|\| event\.key === ' '\)/)
  assert.match(sheetSource, /confirmationSent/)
  assert.match(sheetSource, /touch-action: none;/)
  assert.match(sheetSource, /@media \(prefers-reduced-motion: reduce\)/)
})

test('普通平仓打开弹窗，最终确认冻结比例与幂等键后调用单仓接口', () => {
  const openStart = tradeSource.indexOf('function openMarginCloseSheet')
  const confirmStart = tradeSource.indexOf('async function confirmMarginClose')
  const singleActionStart = tradeSource.indexOf('async function performPositionAction')
  const bulkStart = tradeSource.indexOf('async function performBulkClose')
  assert.ok(openStart >= 0 && confirmStart > openStart && singleActionStart > confirmStart && bulkStart > singleActionStart)
  const openFlow = tradeSource.slice(openStart, confirmStart)
  const confirmFlow = tradeSource.slice(confirmStart, singleActionStart)
  assert.doesNotMatch(openFlow, /closeMarginPosition\(/)
  assert.match(confirmFlow, /createMarginCloseIdempotencyKey\(\)/)
  assert.match(confirmFlow, /await closeMarginPosition\(position\.id, \{[\s\S]*?percentage:[\s\S]*?idempotencyKey:/)
  assert.match(tradeSource, /data-position-action="close"[\s\S]*?aria-haspopup="dialog"[\s\S]*?@click="openMarginCloseSheet\(position, \$event\)"/)
  assert.match(tradeSource, /<MarginCloseSheet[\s\S]*?:open="marginCloseSheetOpen"[\s\S]*?@confirm="confirmMarginClose"[\s\S]*?@close="closeMarginCloseSheet"/)
  assert.match(tradingApiSource, /percentage: input\.percentage/)
  assert.match(tradingApiSource, /idempotency_key: input\.idempotencyKey/)
})

test('平仓弹窗固定文案在中英文资源中保持对称', () => {
  for (const key of [
    'marginCloseTitle',
    'marginCloseMarketPrice',
    'marginCloseLatestPrice',
    'marginCloseQuantity',
    'marginClosePercentage',
    'marginClosePositionAmount',
    'marginCloseAvailableAmount',
    'marginCloseEstimatedPnl',
    'marginCloseSelectedPosition',
    'marginCloseRatioLabel',
    'marginCloseSlideAction',
    'marginCloseSlideProgress',
    'marginCloseSlideReady',
    'positionPartiallyClosed',
  ] as const) {
    assert.equal(typeof zhCN.trade[key], 'string', `zh-CN missing trade.${key}`)
    assert.equal(typeof en.trade[key], 'string', `en missing trade.${key}`)
  }
})
