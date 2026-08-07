import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const ordersSource = readFileSync(new URL('../src/views/OrdersView.vue', import.meta.url), 'utf8')
const swapSource = readFileSync(new URL('../src/views/SwapView.vue', import.meta.url), 'utf8')
const earnSource = readFileSync(new URL('../src/views/EarnView.vue', import.meta.url), 'utf8')
const predictionSource = readFileSync(new URL('../src/views/PredictionView.vue', import.meta.url), 'utf8')
const predictionApiSource = readFileSync(new URL('../src/api/prediction.ts', import.meta.url), 'utf8')
const tradingApiSource = readFileSync(new URL('../src/api/trading.ts', import.meta.url), 'utf8')
const newCoinsSource = readFileSync(new URL('../src/views/NewCoinsView.vue', import.meta.url), 'utf8')
const newCoinDetailSource = readFileSync(new URL('../src/views/NewCoinDetailView.vue', import.meta.url), 'utf8')
const newCoinRecordsSource = readFileSync(new URL('../src/views/NewCoinRecordsView.vue', import.meta.url), 'utf8')
const modalDialogSource = readFileSync(new URL('../src/core/modalDialog.ts', import.meta.url), 'utf8')
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')
const sources = [
  ordersSource,
  swapSource,
  earnSource,
  predictionSource,
  newCoinsSource,
  newCoinDetailSource,
  newCoinRecordsSource,
]

test('订单页保留现货、杠杆查询与逐笔/批量撤单平仓合同', () => {
  assert.match(ordersSource, /data-pencil-source="kcP5D A85if n6oGO t2GTW4 e5Qs1 hxe8l"/)
  assert.match(ordersSource, /<PageHeader :back="false" :pencil="true"/)
  assert.match(ordersSource, /fetchOpenSpotOrders\(\)/)
  assert.match(ordersSource, /fetchSpotOrderHistory\(\)/)
  assert.match(ordersSource, /fetchMarginPositions\('opened'\)/)
  assert.match(ordersSource, /fetchMarginPositions\('closed'\)/)
  assert.match(ordersSource, /fetchMarginPositions\('liquidated'\)/)
  assert.match(ordersSource, /fetchMarginPositions\('canceled'\)/)
  assert.match(ordersSource, /fetchMarketPairs\(\)/)
  assert.match(ordersSource, /product\.id === position\.productId \|\| product\.pairId === position\.pairId/)
  assert.match(ordersSource, /candidate\.id === position\.pairId/)
  assert.doesNotMatch(tradingApiSource, /symbol:\s*String\(position\.[^)]*pair_id/)
  assert.match(ordersSource, /await cancelSpotOrder\(order\.id\)/)
  assert.match(ordersSource, /await cancelAllSpotOrders\(spotOrders\.value\.map\(\(order\) => order\.id\)\)/)
  assert.match(ordersSource, /await cancelMarginPosition\(position\.id\)/)
  assert.match(ordersSource, /await closeMarginPosition\(position\.id\)/)
  assert.match(ordersSource, /await cancelAllMarginPositions\(\)/)
  assert.match(ordersSource, /await closeAllMarginPositions\(\)/)
  assert.match(ordersSource, /route\.query\.tab === 'positions'/)
  assert.match(ordersSource, /route\.query\.tab === 'history'/)
})

test('闪兑与理财页保留实时报价、确认、申购和赎回链路', () => {
  assert.match(swapSource, /data-pencil-source="x9T4CL eXdnN sf288 xvVss"/)
  assert.match(swapSource, /useModalDialog\(pickerOpen, pickerDialog, '\[data-picker-search\]'\)/)
  assert.match(swapSource, /fetchConvertPairs\(\)/)
  assert.match(swapSource, /fetchWalletAccounts\(\), fetchConvertOrders\(\)/)
  assert.match(swapSource, /quote\.value = await requestConvertQuote\(selectedPair\.value, amountNumber\.value\)/)
  assert.match(swapSource, /await confirmConvertQuote\(quote\.value\.quoteId\)/)
  assert.match(swapSource, /quote\.value\.expiresAt <= Date\.now\(\)/)

  assert.match(earnSource, /const productPromise = fetchEarnProducts\(\)/)
  assert.match(earnSource, /Promise\.all\(\[productPromise, fetchEarnSubscriptions\(\), fetchWalletAccounts\(\)\]\)/)
  assert.match(earnSource, /await subscribeEarnProduct\(selected\.value\.id, amountNumber\.value\)/)
  assert.match(earnSource, /await redeemEarnSubscription\(subscription\.id\)/)
  assert.match(earnSource, /amountNumber\.value <= available\.value/)
})

test('预测页保留市场本地化、钱包、报价和订单确认链路', () => {
  assert.match(predictionSource, /fetchPredictionMarkets\(\), fetchPredictionConfig\(\)/)
  assert.match(predictionSource, /fetchWalletAccounts\(\), fetchPredictionOrders\(\)/)
  assert.match(predictionSource, /requestPredictionQuote\(\{ marketId: selected\.value\.id, outcome: outcome\.value, assetId: assetId\.value, stakeAmount: amountNumber\.value \}\)/)
  assert.match(predictionSource, /await confirmPredictionQuote\(quote\.value\.quoteId\)/)
  assert.match(predictionSource, /orders\.value = \[createdOrder, \.\.\.orders\.value\.filter/)
  assert.match(predictionSource, /prediction\.refreshAfterOrderFailed/)
  assert.match(predictionSource, /quote\.value\.expiresAt <= Date\.now\(\)/)
  assert.match(predictionSource, /localizePredictionMarketText\(value, locale\.value, kind\)/)
  assert.match(predictionApiSource, /orderNo: String\(order\.order_no \|\| ''\)/)
  assert.match(predictionApiSource, /confirmPredictionQuote\(quoteId: string\): Promise<PredictionOrder>/)
  assert.match(predictionApiSource, /result: optionalText\(order\.result\)/)
  assert.match(predictionApiSource, /refundAmount: asNumber\(order\.refund_amount\)/)
  assert.doesNotMatch(predictionApiSource, /response\.data\.outcome \|\| 'yes'/)
  assert.match(predictionSource, /prediction\.orderNumber/)
})

test('新币三页保留项目路由、认购购买、四类记录、手续费和释放合同', () => {
  assert.match(newCoinsSource, /fetchNewCoinProjects\(\)/)
  assert.match(newCoinsSource, /fetchNewCoinSubscriptions\(\)/)
  assert.match(newCoinsSource, /router\.push\(\{ name: 'new-coin-detail', params: \{ symbol: project\.symbol \} \}\)/)
  assert.match(newCoinsSource, /router\.push\(\{ name: 'new-coin-records' \}\)/)

  assert.match(newCoinDetailSource, /fetchNewCoinProject\(props\.symbol\)/)
  assert.match(newCoinDetailSource, /fetchWalletAccounts\(\)/)
  assert.match(newCoinDetailSource, /fetchMarketTickers\(\)/)
  assert.match(newCoinDetailSource, /<select v-if="canSubscribe" v-model="quoteAssetId">/)
  assert.match(newCoinDetailSource, /newCoinPurchaseQuantity\(available, value, executionPrice\.value\)/)
  assert.match(newCoinDetailSource, /await subscribeNewCoin\(\{\s*symbol: project\.value\.symbol,\s*quoteAssetId: quoteAssetId\.value,\s*quoteAmount: amountNumber\.value,\s*issuePrice: project\.value\.issuePrice,/)
  assert.match(newCoinDetailSource, /await createNewCoinPurchase\(\{\s*symbol: project\.value\.symbol,\s*pairId: project\.value\.postListingPairId,\s*price: executionPrice\.value,\s*quantity: amountNumber\.value,/)

  for (const request of [
    'fetchNewCoinProjects',
    'fetchNewCoinSubscriptions',
    'fetchNewCoinDistributions',
    'fetchNewCoinPurchases',
    'fetchNewCoinUnlocks',
    'fetchWalletAccounts',
  ]) {
    assert.match(newCoinRecordsSource, new RegExp(`${request}\\(\\)`))
  }
  assert.match(newCoinRecordsSource, /await payNewCoinUnlockFee\(\{\s*idempotencyKey: pendingUnlock\.value\.idempotencyKey,\s*paymentAssetId: paymentAssetId\.value,\s*amount: paymentAmount\.value,/)
  assert.match(newCoinRecordsSource, /await releaseNewCoinUnlock\(unlock\.idempotencyKey\)/)
  assert.match(newCoinRecordsSource, /unlock\.feePaidStatus\.toLowerCase\(\) === 'paid'/)
})

test('资金弹层具备焦点闭环、Escape、滚动锁和主题化遮罩', () => {
  for (const source of [earnSource, predictionSource, newCoinRecordsSource]) {
    assert.match(source, /role="dialog"/)
    assert.match(source, /aria-modal="true"/)
    assert.match(source, /aria-labelledby=/)
    assert.match(source, /data-dialog-cancel/)
    assert.match(source, /background: var\(--overlay\)/)
    assert.match(`${source}\n${selectedCss}`, /:focus-within/)
  }

  for (const source of [earnSource, predictionSource]) {
    assert.match(source, /useModalDialog\(dialogOpen,/)
    assert.match(source, /trap\w+Focus\(event, close\w+\)/)
  }

  assert.match(newCoinRecordsSource, /event\.key === 'Escape'/)
  assert.match(newCoinRecordsSource, /event\.key !== 'Tab'/)
  assert.match(newCoinRecordsSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(modalDialogSource, /event\.key === 'Escape'/)
  assert.match(modalDialogSource, /event\.key !== 'Tab'/)
  assert.match(modalDialogSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(modalDialogSource, /returnFocus\?\.focus\(\)/)
})

test('七个视图遵守主题、Lucide、44px 和窄屏合同', () => {
  for (const source of sources) {
    const contracts = `${source}\n${selectedCss}`
    assert.match(contracts, /min-height:\s*(?:44|4[5-9]|[5-9]\d)px/)
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(contracts, /env\(safe-area-inset-bottom\)/)
    assert.match(contracts, /var\(--(?:surface|page|ink)\)/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(source, /(?:background|color):\s*white\b/i)
    assert.doesNotMatch(source, /rgba?\(/i)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
  }

  for (const source of [swapSource, earnSource, predictionSource, newCoinDetailSource, newCoinRecordsSource]) {
    assert.match(`${source}\n${selectedCss}`, /:focus-within/)
  }
})

test('七个视图使用的静态文案键在中英文资源中均存在', () => {
  const keys = new Set<string>()
  for (const source of sources) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }

  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
