import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const pageHeaderSource = read('../src/components/PageHeader.vue')
const secondsSource = read('../src/views/SecondsView.vue')
const secondsStyle = secondsSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''

test('PageHeader 为 Seconds 提供向后兼容的中间插槽且 Header 只有一个交易对控件', () => {
  assert.match(
    pageHeaderSource,
    /<div class="page-header__copy">\s*<slot name="center">\s*<slot name="copy">[\s\S]*?<strong class="page-header__title">\{\{ title \}\}<\/strong>[\s\S]*?<\/slot>\s*<\/slot>\s*<\/div>/,
  )

  const pageHeader = secondsSource.match(/<PageHeader[\s\S]*?<\/PageHeader>/)?.[0] || ''
  assert.match(pageHeader, /<template #center>[\s\S]*?<label class="field seconds-pair-field">[\s\S]*?<select[\s\S]*?product\.symbol[\s\S]*?<\/template>/)
  assert.match(pageHeader, /<template #actions>[\s\S]*?t\('seconds\.historyTitle'\)[\s\S]*?@click="openHistory"/)
  assert.match(secondsSource, /function openHistory\(\): void \{\s*void router\.push\(\{ name: 'seconds-history' \}\)\s*\}/)
  assert.equal((pageHeader.match(/class="field seconds-pair-field"/g) || []).length, 1)
  assert.equal((secondsSource.match(/class="field seconds-pair-field"/g) || []).length, 1)
  assert.doesNotMatch(secondsSource.slice(secondsSource.indexOf('</PageHeader>') + 13), /class="field seconds-pair-field"/)

  const pairRule = cssRule(secondsStyle, '.seconds-pair-field {')
  assert.match(pairRule, /justify-self: center;/)
  assert.match(pairRule, /max-width: 260px;/)
  assert.match(pairRule, /min-width: 0;/)
  assert.match(pairRule, /width: 100%;/)
  assert.doesNotMatch(pairRule, /position:|left:|right:|top:|z-index:/)

  const shellRule = cssRule(secondsStyle, '.seconds-select-shell {')
  assert.match(shellRule, /grid-template-columns: minmax\(0, 1fr\) auto;/)
  assert.match(shellRule, /height: 44px;/)
  assert.match(shellRule, /min-height: 44px;/)
  assert.match(shellRule, /width: 100%;/)
  const focusRule = cssRule(secondsStyle, '.seconds-select-shell:focus-within {')
  assert.match(focusRule, /box-shadow: inset 0 0 0 1px var\(--focus\);/)
  assert.doesNotMatch(focusRule, /0 0 0 3px/)

  for (const width of [320, 390, 448]) {
    const compactInline = width <= 340 ? 14 : 18
    assert.ok(width - compactInline * 2 - 44 * 2 >= 204, `${width}px header center track must remain usable`)
  }
})

test('Seconds 使用内部 ticker 与 1m K 线会话并保留 REST/WS generation 竞态保护', () => {
  assert.match(secondsSource, /import \{ subscribeTickers \} from '@\/api\/marketSocket'/)
  assert.match(secondsSource, /import \{ createMarketDetailStreamSession \} from '@\/api\/marketDetailStream'/)
  assert.match(secondsSource, /getUrl: publicMarketWebSocketUrl,\s*channels: \['kline'\]/)
  assert.match(secondsSource, /\.\.\.products\.value\.map\(\(product\) => normalizeProductSymbol\(product\.symbol\)\)/)
  assert.match(secondsSource, /\.\.\.activeOrders\.value\.map\(\(order\) => normalizeProductSymbol\(order\.symbol\)\)/)
  assert.match(secondsSource, /stopTickerSubscription = subscribeTickers\(normalizedSymbols, \(update\) => \{/)
  assert.match(secondsSource, /generation !== tickerSubscriptionGeneration/)
  assert.match(secondsSource, /\[update\.symbol\]: update\.lastPrice/)

  assert.match(secondsSource, /secondsKlineSession\.replace\(symbol, '1m', requestVersion\)/)
  assert.match(secondsSource, /secondsKlineSession\.beginKlineRequest\(context\)/)
  assert.match(secondsSource, /fetchKlines\(symbol, '1m'\)/)
  assert.match(secondsSource, /secondsKlineSession\.isCurrent\(context, symbol, '1m', requestVersion\)/)
  assert.match(secondsSource, /secondsKlineSession\.isCurrentKlineRequest\(request\)/)
  assert.match(secondsSource, /secondsKlineSession\.resolveKlineRequest\(request, nextPoints\)/)
  assert.match(secondsSource, /const livePrice = liveTickerPrices\.value\[normalized\][\s\S]*?sparklinePoints\.value\.at\(-1\)\?\.close[\s\S]*?marketStore\.tickerFor\(symbol\)\?\.lastPrice/)

  assert.match(secondsSource, /onBeforeUnmount\(\(\) => \{[\s\S]*?chartRequestVersion \+= 1[\s\S]*?tickerSubscriptionGeneration \+= 1[\s\S]*?secondsKlineSession\.stop\(\)[\s\S]*?stopTickerSubscription\?\.\(\)/)
  assert.doesNotMatch(secondsSource, /https?:\/\/www\.tradingview|<iframe|<script[^>]+src=/i)
})

test('Seconds 渲染全部活动订单、保留并行下单表单并按订单批量到期对账', () => {
  assert.match(secondsSource, /const activeOrders = computed\(\(\) => activeSecondsOrders\(orders\.value\)\)/)
  assert.doesNotMatch(secondsSource, /\bactiveOrder\b|Boolean\(activeOrder\)/)
  assert.match(secondsSource, /v-if="activeOrders\.length"[\s\S]*?data-active-order-list="all"/)
  assert.match(secondsSource, /v-for="order in activeOrders"/)
  assert.match(secondsSource, /:data-active-order-id="order\.id"/)
  assert.match(secondsSource, /order\.symbol[\s\S]*?orderCountdown\(order\)[\s\S]*?orderProgress\(order\)[\s\S]*?order\.entryPrice[\s\S]*?latestPriceForSymbol\(order\.symbol\)[\s\S]*?order\.stakeAmount[\s\S]*?orderEstimatedProfit\(order\)/)

  assert.equal((secondsSource.match(/:disabled="loading \|\| !selected"/g) || []).length, 4)
  assert.match(secondsSource, /:disabled="loading \|\| !selected \|\| value <= 0"/)
  assert.match(secondsSource, /:disabled="submitting \|\| loading \|\| !selected"/)
  assert.match(secondsSource, /async function submit\(\): Promise<void> \{\s*if \(submitting\.value\) return/)

  const mutationStart = secondsSource.indexOf('openedOrder = await openSecondsOrder({')
  const immediateUpsert = secondsSource.indexOf('orders.value = upsertSecondsOrder(orders.value, openedOrder)', mutationStart)
  const successCommit = secondsSource.indexOf("success.value = t('seconds.created')", immediateUpsert)
  const submittingReleased = secondsSource.indexOf('submitting.value = false', successCommit)
  const reconciliation = secondsSource.indexOf('void reconcileOpenedOrder()', submittingReleased)
  assert.ok(
    mutationStart >= 0
    && immediateUpsert > mutationStart
    && successCommit > immediateUpsert
    && submittingReleased > successCommit
    && reconciliation > submittingReleased,
  )
  assert.match(secondsSource, /committedOrdersById\.set\(openedOrder\.id, openedOrder\)/)
  assert.match(secondsSource, /mergeSecondsOrderReconciliation\(nextOrders, committedOrders\)/)
  assert.match(secondsSource, /refreshWarning\.value = t\('seconds\.refreshAfterOrderFailed'\)/)
  assert.doesNotMatch(secondsSource.slice(mutationStart, secondsSource.indexOf('function statusLabel', mutationStart)), /await load\(\)/)

  assert.match(secondsSource, /const expiryRetryAtByOrderId = new Map<number, number>\(\)/)
  assert.match(secondsSource, /const queuedExpiryOrderIds = new Set<number>\(\)/)
  assert.match(secondsSource, /const reconcilingExpiryOrderIds = new Set<number>\(\)/)
  assert.match(secondsSource, /retryAt <= now && !reconcilingExpiryOrderIds\.has\(orderId\)/)
  assert.match(secondsSource, /if \(!queuedExpiryOrderIds\.size \|\| expiryReconciliationPromise\) return/)
  assert.match(secondsSource, /const batch = \[\.\.\.queuedExpiryOrderIds\][\s\S]*?const reconciliation = await reconcilePrivateState\(\)/)
  assert.match(secondsSource, /fetchSecondsOrders\(100\),\s*fetchWalletAccounts\(\)/)
  assert.match(secondsSource, /Date\.now\(\) \+ EXPIRY_RECONCILIATION_RETRY_MS/)
  assert.match(secondsSource, /fullyLoaded && !activeIds\.has\(orderId\)[\s\S]*?expiryRetryAtByOrderId\.delete\(orderId\)/)
  assert.match(secondsSource, /queueExpiredOrderReconciliation\(currentTime\.value\)/)

  assert.equal(zhCN.seconds.activeOrders, '活动订单')
  assert.equal(en.seconds.activeOrders, 'Active orders')
  assert.match(zhCN.seconds.refreshAfterOrderFailed, /订单已创建/)
  assert.match(en.seconds.refreshAfterOrderFailed, /order was created/i)
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function cssRule(source: string, marker: string): string {
  const markerIndex = source.indexOf(marker)
  assert.notEqual(markerIndex, -1, `missing CSS marker: ${marker}`)
  const openIndex = source.indexOf('{', markerIndex)
  const closeIndex = source.indexOf('}', openIndex)
  assert.notEqual(closeIndex, -1, `missing CSS closing brace: ${marker}`)
  return source.slice(openIndex + 1, closeIndex)
}
