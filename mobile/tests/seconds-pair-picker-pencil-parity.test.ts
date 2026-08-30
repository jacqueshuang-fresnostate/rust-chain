import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const secondsSource = read('../src/views/SecondsView.vue')
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')
const secondsStyle = secondsSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''

test('Seconds Header 以 44px 对话框触发器替代透明原生 select', () => {
  const header = secondsSource.match(/<PageHeader[\s\S]*?<\/PageHeader>/)?.[0] || ''

  assert.match(header, /<button[\s\S]*?ref="pairPickerTrigger"[\s\S]*?class="seconds-pair-field"/)
  assert.match(header, /aria-haspopup="dialog"/)
  assert.match(header, /:aria-expanded="pairPickerOpen"/)
  assert.match(header, /aria-controls="seconds-pair-picker"/)
  assert.match(header, /@click="openPairPicker"/)
  assert.match(header, /<strong>\{\{ selectedPairLabel \}\}<\/strong>/)
  assert.doesNotMatch(header, /<select\b|<option\b/)
  assert.doesNotMatch(secondsSource, /selectProductFromEvent/)

  const triggerRule = blockOf(secondsStyle, '.seconds-pair-field {')
  assert.match(triggerRule, /height:\s*44px;/)
  assert.match(triggerRule, /margin:\s*-11px 0;/)
  assert.match(triggerRule, /padding:\s*11px 0;/)
  assert.match(secondsStyle, /\.seconds-pair-field:focus-visible\s*\{[\s\S]*?outline:\s*2px solid var\(--focus\);/)
})

test('Seconds 交易对选择器复用共享模态生命周期并保持产品选择业务不变', () => {
  assert.match(secondsSource, /const pairPickerOpen = ref\(false\)/)
  assert.match(secondsSource, /const pairSearch = ref\(''\)/)
  assert.match(secondsSource, /const pairPickerDialog = ref<HTMLElement \| null>\(null\)/)
  assert.match(secondsSource, /const pairPickerTrigger = ref<HTMLButtonElement \| null>\(null\)/)
  assert.match(secondsSource, /const filteredPairProducts = computed\(\(\) =>/)
  assert.match(
    secondsSource,
    /\} = useModalDialog\(\s*pairPickerOpen,\s*pairPickerDialog,\s*'\[data-seconds-pair-search\]',?\s*\)/,
  )
  assert.match(secondsSource, /setPairPickerReturnFocus\(pairPickerTrigger\.value\)/)
  assert.match(secondsSource, /function handlePairPickerKeydown\(event: KeyboardEvent\): void \{\s*trapPairPickerFocus\(event, closePairPicker\)/)
  assert.match(secondsSource, /function choosePairProduct\(product: SecondsProduct\): void \{\s*selectProduct\(product\)\s*closePairPicker\(\)/)
  assert.match(secondsSource, /function selectProduct\(product: SecondsProduct\): void \{[\s\S]*?void loadSparkline\(product\.symbol\)/)
  assert.doesNotMatch(secondsSource.match(/function selectProduct\([\s\S]*?\n\}/)?.[0] || '', /orders\.value\s*=/)
})

test('Seconds 交易对选择层 1:1 对齐 Pencil 07c 结构与真实数据', () => {
  assert.match(secondsSource, /<Teleport to="body">[\s\S]*?<Transition name="seconds-pair-picker-reveal">/)
  assert.match(secondsSource, /v-if="pairPickerOpen"[\s\S]*?class="seconds-pair-picker-layer"/)
  assert.match(secondsSource, /data-pencil-source="vONcc kLXCs"/)
  assert.match(secondsSource, /@click\.self="closePairPicker"/)
  assert.match(secondsSource, /id="seconds-pair-picker"[\s\S]*?role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(secondsSource, /@keydown="handlePairPickerKeydown"/)
  assert.match(secondsSource, /t\('seconds\.pairPickerTitle'\)/)
  assert.match(secondsSource, /<Search :size="18"/)
  assert.match(secondsSource, /v-model="pairSearch"[\s\S]*?data-seconds-pair-search/)
  assert.match(secondsSource, /t\('seconds\.pairPickerSearchPlaceholder'\)/)
  assert.match(secondsSource, /v-for="product in filteredPairProducts"/)
  assert.match(secondsSource, /role="option"[\s\S]*?:aria-selected="selected\?\.id === product\.id"/)
  assert.match(secondsSource, /<AssetMark[\s\S]*?:src="marketStore\.tickerFor\(product\.symbol\)\?\.baseIconUrl \|\| marketStore\.tickerFor\(product\.symbol\)\?\.iconUrl"[\s\S]*?:size="30"/)
  assert.match(secondsSource, /latestPriceForSymbol\(product\.symbol\)/)
  assert.match(secondsSource, /<Check\s+v-if="selected\?\.id === product\.id"\s+:size="17"/)
  assert.match(secondsSource, /t\('seconds\.pairPickerNoResults'\)/)
  assert.match(secondsSource, /t\('seconds\.pairPickerNote'\)/)
  assert.doesNotMatch(secondsSource, /搜索 BTC、ETH 或 HIPPO|选择后立即切换行情/)
})

test('Seconds 交易对选择层锁定 390x920 几何、长列表滚动和低动态合同', () => {
  const layerRule = blockOf(secondsStyle, '.seconds-pair-picker-layer {')
  assert.match(layerRule, /align-items:\s*flex-end;/)
  assert.match(layerRule, /inset:\s*0;/)
  assert.match(layerRule, /position:\s*fixed;/)

  const sheetRule = blockOf(secondsStyle, '.seconds-pair-picker {')
  assert.match(sheetRule, /border-radius:\s*24px 24px 0 0;/)
  assert.match(sheetRule, /gap:\s*14px;/)
  assert.match(sheetRule, /height:\s*calc\(100dvh - 80px\);/)
  assert.match(sheetRule, /max-height:\s*840px;/)
  assert.match(sheetRule, /max-width:\s*390px;/)
  assert.match(sheetRule, /padding:\s*18px 20px calc\(16px \+ env\(safe-area-inset-bottom\)\);/)

  assert.match(secondsStyle, /\.seconds-pair-picker__header\s*\{[\s\S]*?height:\s*34px;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__title\s*\{[\s\S]*?font-size:\s*20px;[\s\S]*?font-weight:\s*700;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__close\s*\{[\s\S]*?height:\s*44px;[\s\S]*?width:\s*44px;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__close-face\s*\{[\s\S]*?height:\s*34px;[\s\S]*?width:\s*34px;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__search\s*\{[\s\S]*?border-radius:\s*12px;[\s\S]*?height:\s*46px;[\s\S]*?padding:\s*0 14px;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__list\s*\{[\s\S]*?gap:\s*8px;[\s\S]*?overflow-y:\s*auto;[\s\S]*?overscroll-behavior:\s*contain;/)
  assert.match(secondsStyle, /\.seconds-pair-picker__row\s*\{[\s\S]*?border-radius:\s*12px;[\s\S]*?height:\s*64px;[\s\S]*?padding:\s*0 14px;/)
  assert.match(secondsStyle, /@media \(max-height: 640px\)[\s\S]*?\.seconds-pair-picker/)
  assert.match(secondsStyle, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.seconds-pair-picker-reveal-enter-active/)
})

test('Seconds 交易对选择层明暗色板和文案与 Pencil 对称', () => {
  const lightRule = blockOf(
    selectedPageCss,
    ".seconds-pair-picker-layer[data-pencil-source='vONcc kLXCs'] {",
  ).toLowerCase()
  for (const color of [
    '#00000099', '#ffffff', '#111714', '#dde7e1', '#00000022', '#e8f0ec', '#cbd8d1',
    '#25372d', '#f4f7f5', '#ccd5d0', '#68736d', '#d9f9eb', '#43efa9', '#087b52',
  ]) {
    assert.match(lightRule, new RegExp(color))
  }

  const darkRule = blockOf(
    selectedPageCss,
    "html[data-theme='dark'] .seconds-pair-picker-layer[data-pencil-source='vONcc kLXCs'] {",
  ).toLowerCase()
  for (const color of [
    '#000000b8', '#0b0f0d', '#f2f7f4', '#2c3a32', '#00000044', '#1a251f', '#2b3b32',
    '#b4c1ba', '#151f1a', '#29342e', '#95a19a', '#103326', '#43efa9', '#61f1b6', '#0c100e',
  ]) {
    assert.match(darkRule, new RegExp(color))
  }

  const keys = [
    'pairPickerTitle',
    'pairPickerSearchPlaceholder',
    'pairPickerSearchLabel',
    'pairPickerProductsLabel',
    'pairPickerNoResults',
    'pairPickerNote',
  ] as const
  for (const key of keys) {
    assert.equal(typeof zhCN.seconds[key], 'string', `missing zh-CN seconds.${key}`)
    assert.equal(typeof en.seconds[key], 'string', `missing en seconds.${key}`)
  }
})

function blockOf(source: string, marker: string): string {
  const markerIndex = source.indexOf(marker)
  assert.notEqual(markerIndex, -1, `missing block marker: ${marker}`)
  const openIndex = source.indexOf('{', markerIndex)
  assert.notEqual(openIndex, -1, `missing opening brace: ${marker}`)

  let depth = 0
  for (let index = openIndex; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] !== '}') continue
    depth -= 1
    if (depth === 0) return source.slice(openIndex + 1, index)
  }
  assert.fail(`missing closing brace: ${marker}`)
}
