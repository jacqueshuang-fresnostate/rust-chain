import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const secondsSource = read('../src/views/SecondsView.vue')
const secondsFinancialSource = read('../src/core/secondsFinancial.ts')
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')
const secondsTemplate = secondsSource.slice(
  secondsSource.indexOf('<template>') + '<template>'.length,
  secondsSource.lastIndexOf('</template>'),
)
const secondsStyle = secondsSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''

test('Seconds 生产根只声明当前 Pencil 选稿并按 Header、420px 操作区、订单区排序', () => {
  assert.match(secondsSource, /data-pencil-source="VL8er\/g9agt"/)
  assert.doesNotMatch(secondsSource, /Lpt6q|WxeB8/)

  assertOrdered(secondsTemplate, [
    'class="seconds-header"',
    'class="seconds-pair-field"',
    'class="page-content seconds-content"',
    'class="seconds-trading-operation"',
    'class="seconds-market-status"',
    'class="seconds-price-panel"',
    'class="seconds-micro-chart"',
    'class="instrument-plate seconds-order-console"',
    'class="seconds-orders-workspace"',
    'class="seconds-orders-heading"',
    'class="seconds-order-filters"',
    'class="seconds-active-order-list"',
  ])

  const operationRule = blockOf(secondsStyle, '.seconds-trading-operation {')
  assert.match(operationRule, /height:\s*420px;/)
  assert.match(operationRule, /grid-template-rows:\s*22px 53px 112px 202px;/)
  assert.match(operationRule, /padding:\s*2px 20px 10px;/)
  assert.match(operationRule, /row-gap:\s*6px;/)

  const ordersRule = blocksOf(secondsStyle, '.seconds-orders-workspace {').find((rule) =>
    rule.includes('border-top:'),
  ) || ''
  assert.match(ordersRule, /border-top:\s*1px solid var\(--seconds-line\);/)
  assert.match(ordersRule, /padding:\s*12px 20px calc\(16px \+ env\(safe-area-inset-bottom\)\);/)
  assert.doesNotMatch(ordersRule, /max-height|overflow-y:\s*hidden/)
})

test('Seconds Header 使用绝对居中的 140x22 交易对轨道和 40px 视觉控件', () => {
  const header = secondsSource.match(/<PageHeader[\s\S]*?<\/PageHeader>/)?.[0] || ''
  assert.match(header, /class="seconds-header"/)
  assert.match(header, /<strong>\{\{ selectedPairLabel \}\}<\/strong>/)
  assert.match(header, /<small>\{\{ t\('seconds\.title'\) \}\}<\/small>/)
  assert.match(header, /<ChevronDown :size="15"/)
  assert.match(header, /<button[\s\S]*?class="seconds-pair-field"[\s\S]*?aria-haspopup="dialog"[\s\S]*?@click="openPairPicker"/)
  assert.doesNotMatch(header, /<select\b|<option\b/)
  assert.match(header, /<History :size="18"/)

  const copyRule = blockOf(secondsStyle, '.seconds-header :deep(.page-header__copy) {')
  assert.match(copyRule, /height:\s*22px;/)
  assert.match(copyRule, /left:\s*50%;/)
  assert.match(copyRule, /position:\s*absolute;/)
  assert.match(copyRule, /transform:\s*translateX\(-50%\);/)
  assert.match(copyRule, /width:\s*140px;/)

  const headerRule = blockOf(secondsStyle, '.seconds-header {')
  assert.match(headerRule, /grid-template-columns:\s*40px minmax\(0, 1fr\) 40px(?: !important)?;/)
  assert.match(headerRule, /height:\s*60px(?: !important)?;/)
  assert.match(headerRule, /padding:\s*10px 20px(?: !important)?;/)
  assert.match(secondsStyle, /\.seconds-header :deep\(\.page-header__actions\)\s*\{[\s\S]*?grid-column:\s*3;/)
  assert.match(secondsStyle, /\.seconds-header :deep\(\.icon-button\)::before\s*\{[\s\S]*?inset:\s*-2px;/)
})

test('Seconds 行情摘要只消费实时快照、真实订单轮次和 112px 四线图表', () => {
  assert.match(secondsSource, /type TickerUpdate/)
  assert.match(secondsSource, /const liveTickerSnapshots = ref<Record<string, TickerUpdate>>\(\{\}\)/)
  assert.match(secondsSource, /update\.changePercent/)
  assert.match(secondsSource, /update\.observedAt/)
  assert.match(secondsSource, /const selectedChangePercent = computed\(\(\) => displayChangePercent\([\s\S]*?selectedLiveTicker\.value,[\s\S]*?selectedTicker\.value/)
  assert.match(secondsFinancialSource, /const displayChangePercent = \([\s\S]*?finiteDisplayNumber\(liveTicker\?\.changePercent\)[\s\S]*?finiteDisplayNumber\(snapshotTicker\?\.changePercent\)/)
  assert.doesNotMatch(secondsSource, /if \(selectedLiveTicker\.value\) return Number\.isFinite\(liveChange\)/)
  assert.match(secondsSource, /const nearestSelectedActiveOrder = computed/)
  assert.match(secondsSource, /nearestSelectedActiveOrder\.value[\s\S]*?orderCountdown\(order\)/)
  assert.match(secondsSource, /t\('seconds\.readyState'\)/)
  assert.match(secondsTemplate, /selectedChangePercent[\s\S]*?formatPercent\(selectedChangePercent\)/)
  assert.match(secondsTemplate, /t\('seconds\.referencePrice'\)/)
  assert.match(secondsTemplate, /t\('common\.liveData'\)/)
  assert.match(secondsStyle, /\.seconds-price-panel > strong\s*\{[\s\S]*?color:\s*var\(--seconds-positive-text\);/)
  assert.doesNotMatch(secondsTemplate, /class="seconds-live-state"[\s\S]{0,500}?<i aria-hidden="true"/)

  const chartRule = blockOf(secondsStyle, '.seconds-micro-chart {')
  assert.match(chartRule, /height:\s*112px;/)
  assert.match(secondsStyle, /\.seconds-micro-chart canvas\s*\{[\s\S]*?height:\s*112px;/)
  assert.match(secondsSource, /for \(let index = 0; index < 4; index \+= 1\)/)
  assert.match(secondsSource, /context\.lineWidth = 2/)
  assert.match(secondsSource, /createRadialGradient/)
  assert.match(secondsSource, /context\.arc\([\s\S]*?12, 0, Math\.PI \* 2\)/)
  assert.match(secondsSource, /context\.arc\([^\n]+4\.5, 0, Math\.PI \* 2\)/)

  assert.doesNotMatch(secondsSource, /(?:63,?085|01842|mock|fixture|demoData|fakeOrder)/i)
})

test('Seconds 202px 下单表单锁定期限、限额、金额、方向和 44px 主操作几何', () => {
  assertOrdered(secondsTemplate, [
    'class="seconds-duration-scroll"',
    'class="seconds-cycle-limit"',
    'seconds-amount-field',
    'class="seconds-direction-grid"',
    'class="button button--primary button--full seconds-submit"',
  ])

  const consoleRule = blockOf(secondsStyle, '.seconds-order-console {')
  assert.match(consoleRule, /gap:\s*6px;/)
  assert.match(consoleRule, /grid-template-rows:\s*30px 26px 38px 40px 44px;/)
  assert.match(consoleRule, /height:\s*202px;/)

  assert.match(secondsStyle, /\.seconds-duration-grid\s*\{[\s\S]*?grid-auto-columns:\s*calc\(\(100% - 18px\) \/ 4\);[\s\S]*?grid-auto-flow:\s*column;/)
  assert.match(secondsStyle, /\.seconds-duration-grid\s*\{[\s\S]*?grid-template-columns:\s*none;/)
  assert.match(secondsStyle, /\.seconds-duration-scroll\s*\{[\s\S]*?overflow-x:\s*auto;/)
  assert.match(secondsStyle, /\.seconds-duration-grid button\s*\{[\s\S]*?border-radius:\s*9px;[\s\S]*?height:\s*30px;[\s\S]*?min-height:\s*30px(?: !important)?;/)
  assert.match(secondsStyle, /\.seconds-cycle-limit\s*\{[\s\S]*?border-radius:\s*8px;[\s\S]*?height:\s*26px;/)
  assert.match(secondsStyle, /\.seconds-cycle-limit\s*\{[\s\S]*?background:\s*var\(--seconds-positive-soft\);/)
  assert.match(secondsStyle, /\.seconds-amount-field\s*\{[\s\S]*?border-radius:\s*10px;[\s\S]*?height:\s*38px;/)
  assert.match(secondsStyle, /\.seconds-direction-grid button\s*\{[\s\S]*?border-radius:\s*14px;[\s\S]*?height:\s*40px;[\s\S]*?min-height:\s*40px(?: !important)?;/)
  assert.match(secondsStyle, /\.seconds-direction-grid button\.up\.active\s*\{[\s\S]*?box-shadow:\s*none;/)
  assert.match(secondsStyle, /\.seconds-submit\s*\{[\s\S]*?border-radius:\s*10px;[\s\S]*?height:\s*44px;/)
  assert.match(secondsStyle, /\.seconds-submit\s*\{[\s\S]*?box-shadow:\s*none !important;/)
  assert.match(secondsTemplate, /:class="\{ 'seconds-submit--down': direction === 'down' \}"/)
  assert.match(secondsStyle, /\.seconds-submit\.seconds-submit--down\s*\{[\s\S]*?background:\s*var\(--seconds-negative\) !important;/)

  assert.match(secondsTemplate, /:aria-pressed="direction === 'up'"/)
  assert.match(secondsTemplate, /:aria-pressed="direction === 'down'"/)
  assert.match(secondsTemplate, /\{\{ submitting \? t\('common\.submitting'\) : orderActionLabel \}\}/)
  assert.match(secondsSource, /orderReview\.value\?\.estimatedProfit \?\? null/)
  assert.match(secondsSource, /const availableStakeBalance = computed\(\(\) => walletAvailable\(account\.value\)\)/)
  assert.match(secondsSource, /validateStake\(amount\.value,[\s\S]*?available: availableStakeBalance\.value/)
  assert.match(secondsSource, /const amountFieldInvalid = computed\(\(\) => Boolean\([\s\S]*?session\.isAuthenticated[\s\S]*?!loading\.value[\s\S]*?!valid\.value/)
  assert.match(secondsTemplate, /:data-field-state="amountFieldInvalid \? 'invalid' : amount && valid \? 'complete' : 'idle'"[\s\S]*?:aria-invalid="amountFieldInvalid"/)
})

test('Seconds 活动订单使用真实 Logo、本地筛选和可自然增长的 350x82 卡片', () => {
  assert.match(secondsSource, /import AssetMark from '@\/components\/AssetMark\.vue'/)
  assert.match(secondsSource, /const activeOrderFilter = ref<'all' \| 'up' \| 'down'>\('all'\)/)
  assert.match(secondsSource, /const filteredActiveOrders = computed/)
  assert.match(secondsTemplate, /@click="activeOrderFilter = 'all'"/)
  assert.match(secondsTemplate, /@click="activeOrderFilter = 'up'"/)
  assert.match(secondsTemplate, /@click="activeOrderFilter = 'down'"/)
  assert.match(secondsTemplate, /t\('common\.all'\)\s*\}\}\s*\{\{ activeOrderCounts\.all \}\}/)
  assert.match(secondsTemplate, /t\('seconds\.bullish'\)\s*\}\}\s*\{\{ activeOrderCounts\.up \}\}/)
  assert.match(secondsTemplate, /t\('seconds\.bearish'\)\s*\}\}\s*\{\{ activeOrderCounts\.down \}\}/)
  assert.match(secondsTemplate, /<ChevronRight :size="14"/)
  assert.match(secondsStyle, /\.seconds-orders-heading > button\s*\{[\s\S]*?height:\s*24px;[\s\S]*?min-height:\s*24px(?: !important)?;/)
  assert.match(secondsStyle, /\.seconds-order-filters button\s*\{[\s\S]*?height:\s*30px;[\s\S]*?min-height:\s*30px(?: !important)?;/)
  assert.match(secondsTemplate, /v-for="order in filteredActiveOrders"/)
  assert.match(secondsTemplate, /<AssetMark[\s\S]*?:src="marketStore\.tickerFor\(order\.symbol\)\?\.baseIconUrl \|\| marketStore\.tickerFor\(order\.symbol\)\?\.iconUrl"[\s\S]*?:size="22"/)
  assert.match(secondsTemplate, /moneyText\(orderMoney\(order\)\.entryPrice\)/)
  assert.match(secondsTemplate, /moneyText\(orderProfit\(order\)\)/)

  const cardRule = blockOf(secondsStyle, '.seconds-active-order {')
  assert.match(cardRule, /border:\s*1px solid var\(--seconds-line\);/)
  assert.match(cardRule, /border-radius:\s*12px;/)
  assert.match(cardRule, /height:\s*82px;/)
  assert.match(cardRule, /padding:\s*8px 10px;/)
  assert.match(secondsStyle, /\.seconds-active-progress\s*\{[\s\S]*?height:\s*3px;/)

  const listRule = blockOf(secondsStyle, '.seconds-active-order-list {')
  assert.match(listRule, /display:\s*grid;/)
  assert.match(listRule, /gap:\s*8px;/)
  assert.doesNotMatch(listRule, /max-height|overflow:\s*hidden/)

  const workspaceRule = blocksOf(secondsStyle, '.seconds-orders-workspace {').find((rule) =>
    rule.includes('border-top:'),
  ) || ''
  assert.match(workspaceRule, /align-content:\s*start;/)
  assert.match(workspaceRule, /min-height:\s*362px;/)
})

test('Seconds 浅深主题精确声明 VL8er/g9agt 颜色且文案中英文对称', () => {
  const lightRule = blockOf(selectedPageCss, '.app-stage .mobile-canvas .seconds-page {')
  for (const color of ['#ffffff', '#111714', '#68736d', '#dde4e0', '#d9f9eb', '#087b52', '#43efa9', '#ff654a']) {
    assert.match(lightRule.toLowerCase(), new RegExp(color))
  }

  const darkRule = blockOf(
    selectedPageCss,
    "html[data-theme='dark'] .app-stage .mobile-canvas .seconds-page {",
  )
  for (const color of ['#000000', '#050806', '#0c100e', '#f2f7f4', '#95a19a', '#202923', '#103326', '#61f1b6', '#43efa9', '#ff654a']) {
    assert.match(darkRule.toLowerCase(), new RegExp(color))
  }

  const keys = [
    'readyState',
    'activeRoundStatus',
    'returnRate',
    'cycleLimitRange',
    'cycleLimitMinimum',
    'cycleOrderLimit',
    'orderAction',
    'inProgressOrders',
    'allOrders',
    'activeOrdersEmpty',
  ] as const
  for (const key of keys) {
    assert.equal(typeof zhCN.seconds[key], 'string', `missing zh-CN seconds.${key}`)
    assert.equal(typeof en.seconds[key], 'string', `missing en seconds.${key}`)
  }
  assert.doesNotMatch(secondsTemplate, /[\u3400-\u9fff]/u)
  assert.doesNotMatch(secondsTemplate, /<svg|\p{Extended_Pictographic}/u)
  assert.match(selectedPageCss, /\.app-stage \.mobile-canvas \.seconds-page\.page\s*\{[\s\S]*?background-image:\s*none;/)
  assert.match(selectedPageCss, /\.app-stage \.mobile-canvas \.seconds-page \.seconds-order-console\s*\{[\s\S]*?background:\s*transparent;/)
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

function blocksOf(source: string, marker: string): string[] {
  const blocks: string[] = []
  let cursor = 0
  while (cursor < source.length) {
    const markerIndex = source.indexOf(marker, cursor)
    if (markerIndex === -1) break
    const block = blockOf(source.slice(markerIndex), marker)
    blocks.push(block)
    cursor = markerIndex + marker.length
  }
  return blocks
}

function assertOrdered(source: string, markers: readonly string[]): void {
  let cursor = -1
  for (const marker of markers) {
    const next = source.indexOf(marker, cursor + 1)
    assert.ok(next > cursor, `expected ${marker} after index ${cursor}`)
    cursor = next
  }
}
