import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
const secondsSource = readFileSync(new URL('../src/views/SecondsView.vue', import.meta.url), 'utf8')
const loanSource = readFileSync(new URL('../src/views/LoanView.vue', import.meta.url), 'utf8')
const walletSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const prototypeCss = readFileSync(new URL('../src/styles/prototype-base.css', import.meta.url), 'utf8')
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')

test('交易页保留现货、合约和 mode 路由合同', () => {
  assert.match(tradeSource, /await placeSpotOrder\(\{\s*symbol: pairSymbol\.value,\s*side: side\.value,\s*type: submittedOrderType,\s*price: limitPrice,\s*quantity: orderAmount,/)
  assert.match(tradeSource, /createMarginOrderReview\(\{\s*productId: selectedProduct\.value\?\.id \|\| 0,\s*side: side\.value,\s*marginMode: marginMode\.value,\s*leverage: leverage\.value,\s*marginAmount: Number\(quantity\.value\),/)
  assert.match(tradeSource, /await placeMarginOrder\(review\.request\)/)
  assert.match(tradeSource, /await updateMarginLeverage\(product\.id, nextLeverage\)/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.match(tradeSource, /navigation\.rememberTradeMode\(mode\.value\)/)
  assert.match(tradeSource, /mode\.value !== 'contract' \|\| !session\.isAuthenticated/)
  assert.match(tradeSource, /function reviewOrder\(event\?: Event\): void \{[\s\S]*?if \(!session\.isAuthenticated\) \{\s*openLogin\(\)/)
  assert.match(tradeSource, /percentage: percent \/ 100/)
  assert.match(tradeSource, /mode\.value === 'contract' && !selectedProduct\.value/)
  assert.match(tradeSource, /t\('trade\.marginField', \{ asset: availableAsset \}\)/)
  assert.match(tradeSource, /t\('rootPrototype\.estimatedNotional'\)/)
  assert.match(tradeSource, /class="contract-order-type"[\s\S]*?:aria-expanded="contractSheet === 'orderType'"[\s\S]*?@click="openContractSheet\('orderType'\)"/)
  assert.match(tradeSource, /v-else-if="balancesError"[\s\S]*?@click="loadTradingBalances"/)
  assert.doesNotMatch(tradeSource, /percentage: percent,\s*price:/)
  assert.doesNotMatch(tradeSource, /marginAmount:\s*Number\(amountValue/)
  assert.match(tradeSource, /class="trade-console"/)
  assert.doesNotMatch(tradeSource, /v-if="mode === 'contract' && !session\.isAuthenticated"/)
  assert.doesNotMatch(tradeSource, /class="trade-category"/)
  assert.doesNotMatch(tradeSource, /selectTradeMode/)
  assert.match(tradeSource, /:data-trade-mode="mode"/)
  assert.match(tradeSource, /class="chart-semantic-summary"/)
})

test('秒合约页保留真实产品、钱包、下单和活动订单合同', () => {
  assert.match(secondsSource, /const nextProducts = await fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /const privateStatePromise:[\s\S]*?session\.isAuthenticated[\s\S]*?Promise\.allSettled\(\[fetchSecondsOrders\(100\), fetchWalletAccounts\(\)\]\)/)
  assert.match(
    secondsSource,
    /if \(!privateResults\) \{\s*clearSecondsPrivateState\(\)\s*replaceTickerSubscription\(\)\s*return\s*\}/,
  )
  assert.match(
    secondsSource,
    /function clearSecondsPrivateState\(\): void \{\s*orders\.value = \[\]\s*accounts\.value = \[\][\s\S]*?settlementResultTracker\.reset\(\)[\s\S]*?clearSettlementResultQueue\(\)\s*\}/,
  )
  assert.match(secondsSource, /await openSecondsOrder\(\{\s*productId: selected\.value\.id,\s*durationSeconds: cycle\.value\.durationSeconds,\s*direction: direction\.value,\s*stakeAmount: amountNumber\.value,/)
  assert.match(secondsSource, /class="seconds-direction-grid"/)
  assert.match(secondsSource, /class="seconds-duration-grid"/)
  assert.match(secondsSource, /class="[^"]*seconds-amount-field[^"]*"/)
  assert.match(secondsSource, /data-active-order-list="all"/)
  assert.match(secondsSource, /selected && session\.isAuthenticated && account/)
  assert.match(secondsSource, /router\.push\(\{ name: 'seconds-history' \}\)/)
  assert.doesNotMatch(secondsSource, /seconds-session-records|seconds-orders|ordersSection|scrollToOrders/)
  assert.doesNotMatch(secondsSource, /cancelSecondsOrder|\/seconds-contracts\/orders\/\$\{[^}]+\}\/cancel/)
})

test('借贷页保留真实申请、撤销、还款并开放逾期还款', () => {
  assert.match(loanSource, /await applyLoan\(\{[\s\S]*productId: selected\.value\.id,[\s\S]*amount: amountNumber\.value,[\s\S]*collateralAssetId:[\s\S]*collateralAmount:/)
  assert.match(loanSource, /await cancelLoanOrder\(order\.id\)/)
  assert.match(loanSource, /await repayLoanOrder\(order\.id\)/)
  assert.match(loanSource, /status === 'disbursed' \|\| status === 'overdue'/)
  assert.match(loanSource, /requestOrderAction\(order\)/)
  assert.match(loanSource, /confirmOrderAction/)
  assert.match(loanSource, /return amountNumber\.value \* product\.interestRate/)
  assert.doesNotMatch(loanSource, /product\.interestRate \* product\.termDays \/ 365/)
  assert.match(loanSource, /function statusLabel\(status: string\)/)
  assert.match(loanSource, /const productsReady = ref\(false\)/)
  assert.match(loanSource, /v-else-if="loading && !hasProducts"[\s\S]*?t\('loan\.loading'\)/)
  assert.match(loanSource, /v-if="!visibleProducts\.length"[\s\S]*?t\('loan\.noProducts'\)/)
})

test('借贷抵押资产使用带后端 Logo 的可访问底部弹窗', () => {
  assert.doesNotMatch(loanSource, /<select\b/)
  assert.match(loanSource, /class="loan-collateral-trigger"[\s\S]*?aria-haspopup="dialog"[\s\S]*?:aria-expanded="collateralPickerOpen"/)
  assert.match(loanSource, /<AssetMark v-if="selectedCollateral" :symbol="selectedCollateral\.symbol" :src="selectedCollateral\.logoUrl"/)
  assert.match(loanSource, /<Teleport to="body">[\s\S]*?class="pencil-sheet-mask loan-collateral-mask"/)
  assert.match(loanSource, /id="loan-collateral-picker"[\s\S]*?role="dialog"[\s\S]*?aria-modal="true"[\s\S]*?aria-labelledby="loan-collateral-picker-title"/)
  assert.match(loanSource, /v-for="account in accounts"[\s\S]*?<AssetMark :symbol="account\.symbol" :src="account\.logoUrl"/)
  assert.match(loanSource, /:aria-pressed="account\.assetId === collateralAssetId"/)
  assert.match(loanSource, /function selectCollateralAsset\(account: WalletAccount\): void \{[\s\S]*?collateralAssetId\.value = account\.assetId[\s\S]*?closeCollateralPicker\(\)/)
  assert.match(loanSource, /const modalOpen = computed\(\(\) => dialogOpen\.value \|\| collateralPickerOpen\.value\)/)
  assert.match(loanSource, /watch\(modalOpen,[\s\S]*?document\.body\.style\.overflow = 'hidden'[\s\S]*?returnFocus\?\.focus\(\)/)
  assert.match(loanSource, /trapDialogFocus\(event, closeCollateralPicker\)/)
  assert.match(loanSource, /@click\.self="closeCollateralPicker"/)
  assert.match(walletSource, /logoUrl: account\.logo_url\?\.trim\(\) \|\| undefined/)
  assert.equal(zhCN.loan.selectCollateralAsset, '选择抵押资产')
  assert.equal(en.loan.selectCollateralAsset, 'Select collateral asset')
  assert.equal(zhCN.loan.availableBalance, '可用 {amount}')
  assert.equal(en.loan.availableBalance, 'Available {amount}')
})

test('三页满足聚焦、确认层、触控和窄屏合同', () => {
  for (const source of [secondsSource, loanSource]) {
    const contracts = source === loanSource ? `${source}\n${selectedCss}` : source
    assert.match(contracts, /:focus-within/)
    assert.match(contracts, /min-height:\s*(?:44|4[5-9]|[5-9]\d)px/)
    assert.match(contracts, /env\(safe-area-inset-/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }

  assert.doesNotMatch(tradeSource, /<style scoped|<svg|\p{Extended_Pictographic}/u)
  assert.match(prototypeCss, /\.input-stack label:focus-within\s*\{/)
  assert.match(prototypeCss, /\.submit-order\s*\{[\s\S]*?min-height:\s*50px/)
  assert.match(prototypeCss, /\.view\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/)
  assert.match(secondsSource, /role="dialog"/)
  assert.match(secondsSource, /aria-modal="true"/)
  assert.match(secondsSource, /event\.key === 'Escape'/)
  assert.match(secondsSource, /event\.key !== 'Tab'/)
  assert.match(loanSource, /role="dialog"/)
  assert.match(loanSource, /aria-modal="true"/)
  assert.match(loanSource, /event\.key === 'Escape'/)
  assert.match(loanSource, /event\.key !== 'Tab'/)
  assert.match(prototypeCss, /@media \(max-width: 350px\)/)
  assert.match(secondsSource, /@media \(max-width: 340px\)/)
  assert.match(loanSource, /@media \(max-width: 340px\)/)
  assert.match(loanSource, /@media \(max-width: 340px\)/)
})

test('三个视图使用的静态文案键在中英文资源中均存在', () => {
  const keys = new Set<string>()
  for (const source of [tradeSource, secondsSource, loanSource]) {
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
