import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const sources = {
  newCoinRecords: read('../src/views/NewCoinRecordsView.vue'),
  assets: read('../src/views/AssetsView.vue'),
  help: read('../src/views/HelpSupportView.vue'),
  profile: read('../src/views/ProfileView.vue'),
  rootHeader: read('../src/components/RootHeader.vue'),
  orders: read('../src/views/OrdersView.vue'),
  ledger: read('../src/views/WalletLedgerView.vue'),
  messages: read('../src/views/MessageCenterView.vue'),
  prediction: read('../src/views/PredictionView.vue'),
  earn: read('../src/views/EarnView.vue'),
  walletApi: read('../src/api/wallet.ts'),
  earnApi: read('../src/api/earn.ts'),
  modalDialog: read('../src/core/modalDialog.ts'),
  router: read('../src/router/index.ts'),
}

test('all eight saved light and dark Pencil pairs are declared by production roots', () => {
  const expected = [
    [sources.newCoinRecords, 'A9It6g h4gfd'],
    [sources.assets, 'v6phV TuWXq'],
    [sources.help, 'UouET FM5tp'],
    [sources.orders, 'e5Qs1 hxe8l'],
    [sources.ledger, 'Bcug6 IVMAO'],
    [sources.messages, 't7j6n eSMHf'],
    [sources.prediction, 'CzpTv ZvGMv'],
    [sources.earn, 'nqP6W aXxul'],
  ] as const

  for (const [source, pair] of expected) {
    const [light, dark] = pair.split(' ')
    assert.match(source, new RegExp(`data-pencil-source="[^"]*${light} ${dark}[^"]*"`))
  }
})

test('help route fixes Profile intent and exposes only configured channels', () => {
  assert.match(sources.router, /const HelpSupportView = \(\) => import\('@\/views\/HelpSupportView\.vue'\)/)
  assert.match(sources.router, /path: '\/profile\/help', name: 'help-support', component: HelpSupportView, meta: \{ showBottomNav: false, depth: 1, backFallback: '\/profile' \}/)
  assert.match(sources.profile, /router\.push\(\{ name: 'help-support' \}\)[\s\S]*?t\('profile\.helpSupport'\)/)
  assert.doesNotMatch(sources.profile, /name: 'message-center'/)
  assert.match(sources.rootHeader, /router\.push\(\{ name: 'message-center' \}\)/)

  assert.match(sources.help, /const filteredFaqs = computed/)
  assert.match(sources.help, /expandedFaqId\.value === id \? '' : id/)
  assert.match(sources.help, /import\.meta\.env\.VITE_SUPPORT_CHAT_URL/)
  assert.match(sources.help, /import\.meta\.env\.VITE_SUPPORT_EMAIL/)
  assert.match(sources.help, /:disabled="!supportChatUrl"/)
  assert.match(sources.help, /:disabled="!supportEmail"/)
  assert.doesNotMatch(sources.help, /support@hippo|24\s*\/\s*7|7\s*[×x]\s*24/i)
  assert.match(sources.help, /height: 44px;/)
  assert.match(sources.help, /height: 64px;/)
})

test('selected empty branches have independent 56px plates, two-line copy, and truthful retry behavior', () => {
  assert.match(sources.orders, /<ClipboardList :size="24"/)
  assert.match(sources.orders, /class="orders-empty-state" role="status"/)
  assert.match(sources.orders, /orders\.emptyDescription/)
  assert.match(sources.orders, /class="orders-empty-action"[\s\S]*?orders\.goTrade/)
  assert.match(sources.orders, /\.orders-empty-state__plate[\s\S]*?height: 56px;[\s\S]*?width: 56px;/)
  assert.match(sources.orders, /\.orders-empty-action[\s\S]*?height: 50px;/)
  assert.equal(sources.orders.match(/v-else-if="!error" class="orders-empty-branch"/g)?.length, 4)

  assert.match(sources.ledger, /<FileSearch :size="24"/)
  assert.match(sources.ledger, /ledger\.emptyDescription/)
  assert.match(sources.ledger, /v-if="error && !entries\.length"/)
  assert.match(sources.ledger, /v-if="error && entries\.length"/)
  assert.match(sources.ledger, /\.ledger-state__plate[\s\S]*?height: 56px;[\s\S]*?width: 56px;/)

  assert.match(sources.messages, /<BellOff :size="24"/)
  assert.match(sources.messages, /class="message-empty-state" role="status"/)
  assert.match(sources.messages, /\.message-empty-state__plate[\s\S]*?height: 56px;[\s\S]*?width: 56px;/)
  assert.match(sources.messages, /<strong>\{\{ emptyTitle \}\}<\/strong>[\s\S]*?<small>\{\{ emptyDescription \}\}<\/small>/)
})

test('orders CTA opens the persisted spot pair without merging trade modes', () => {
  assert.match(sources.orders, /const navigation = useNavigationStore\(\)/)
  assert.match(sources.orders, /router\.push\(\{ name: 'trade', params: \{ symbol: navigation\.lastTradeSymbol \} \}\)/)
  const openSpotTrade = sources.orders.match(/function openSpotTrade\(\): void \{([\s\S]*?)\n\}/)?.[1] || ''
  assert.doesNotMatch(openSpotTrade, /mode|contract|seconds/)
})

test('transfer sheet keeps live wallet APIs, 460px geometry, and dialog accessibility', () => {
  assert.match(sources.assets, /fetchWalletAccounts\(\)/)
  assert.match(sources.assets, /fetchMarginWallets\(\)/)
  assert.match(sources.assets, /await transferWalletFunds\(transferAsset\.value, transferFrom\.value, to, transferValue\)/)
  assert.match(sources.assets, /const transferAvailable = computed<number \| null>\(\(\) => transferAccount\.value\?\.available \?\? null\)/)
  assert.match(sources.assets, /transferAvailable\.value !== null/)
  assert.match(sources.assets, /const transferTarget = computed<'spot' \| 'margin'>/)
  assert.match(sources.assets, /accounts\.value = upsertWalletAccount\(accounts\.value, \{ \.\.\.result\.spotWallet/)
  assert.match(sources.assets, /marginAccounts\.value = upsertWalletAccount\(marginAccounts\.value, \{ \.\.\.result\.marginWallet/)
  assert.match(sources.assets, /class="assets-transfer-route"/)
  assert.match(sources.assets, /assets\.availableBalance/)
  assert.match(sources.assets, /assets\.transferAmount/)
  assert.match(sources.assets, /swapTransferRoute/)
  assert.match(sources.assets, /role="dialog"/)
  assert.match(sources.assets, /aria-modal="true"/)
  assert.match(sources.assets, /useModalDialog\(transferOpen, transferDialog\)/)
  assert.match(sources.assets, /<Teleport to="body">[\s\S]*?class="confirmation-layer assets-transfer-layer"/)
  assert.match(sources.assets, /\.assets-transfer-layer \{[\s\S]*?inset: 0;[\s\S]*?position: fixed;[\s\S]*?width: 100%;/)
  assert.match(sources.assets, /height: min\(460px,/)
  assert.match(sources.assets, /\.assets-transfer-submit[\s\S]*?height: 50px;/)

  assert.match(sources.walletApi, /idempotency_key: createWalletMutationIdempotencyKey\('mobile-transfer'\)/)
  assert.match(sources.walletApi, /spotWallet: mapTransferWallet\(response\.data\.spot_wallet, symbol\)/)
  assert.match(sources.walletApi, /marginWallet: mapTransferWallet\(response\.data\.margin_wallet, symbol\)/)
})

test('prediction and earn confirmations render only API-derived quote and product values', () => {
  assert.match(sources.prediction, /fetchPredictionMarkets\(\), fetchPredictionConfig\(\)/)
  assert.match(sources.prediction, /fetchWalletAccounts\(\), fetchPredictionOrders\(\)/)
  assert.match(sources.prediction, /requestPredictionQuote\(\{ marketId: selected\.value\.id, outcome: outcome\.value, assetId: assetId\.value, stakeAmount: amountNumber\.value \}\)/)
  assert.match(sources.prediction, /await confirmPredictionQuote\(quote\.value\.quoteId\)/)
  assert.match(sources.prediction, /selected\.yesPrice \* 100/)
  assert.match(sources.prediction, /selected\.noPrice \* 100/)
  assert.match(sources.prediction, /quote\.theoreticalPayout/)
  assert.match(sources.prediction, /selected\.value\?\.settlementStatus/)
  assert.match(sources.prediction, /return status \? statusLabel\(status\) : '--'/)
  assert.doesNotMatch(sources.prediction, /prediction\.settlementPending/)
  assert.match(sources.prediction, /prediction\.riskNotice/)
  assert.match(sources.prediction, /role="dialog"/)
  assert.match(sources.prediction, /useModalDialog\(dialogOpen, orderDialog, '\[data-dialog-cancel\]'\)/)
  assert.match(sources.prediction, /selectedAccount\.value[\s\S]*?amountNumber\.value <= selectedAccount\.value\.available/)
  assert.match(sources.prediction, /return account \? formatAmount\(account\.available\) : '--'/)
  assert.match(sources.prediction, /:placeholder="t\('prediction\.amountPlaceholder'\)"/)
  assert.doesNotMatch(sources.prediction, /placeholder="0\.00"/)

  assert.match(sources.earn, /fetchEarnProducts\(\)/)
  assert.match(sources.earn, /fetchEarnSubscriptions\(\)/)
  assert.match(sources.earn, /fetchWalletAccounts\(\)/)
  assert.match(sources.earn, /await subscribeEarnProduct\(selected\.value\.id, amountNumber\.value\)/)
  assert.match(sources.earn, /await redeemEarnSubscription\(subscription\.id\)/)
  assert.match(sources.earn, /amountNumber\.value \* selected\.value\.aprRate \/ 365/)
  assert.match(sources.earn, /earn\.interestStartRule/)
  assert.match(sources.earn, /earn\.redemptionRule/)
  assert.match(sources.earn, /earn\.ruleUnavailable/)
  assert.match(sources.earn, /useModalDialog\(dialogOpen, subscribeDialog, '\[data-dialog-cancel\]'\)/)
  assert.match(sources.earn, /available\.value !== null/)
  assert.match(sources.earn, /earn\.availabilityWithMaximum/)
  assert.match(sources.earn, /Math\.min\(available\.value, productMaximum\)/)
  assert.match(sources.earn, /:disabled="submitting \|\| !canSubscribe"/)
  assert.match(sources.earn, /const selectedRedemptionRule = computed/)
  assert.match(sources.earn, /\.earn-dialog-submit[\s\S]*?height: 50px;/)
  assert.match(sources.earn, /role="dialog"/)

  for (const field of [
    'redemption_fee_rate',
    'maturity_profit_fee_rate',
    'early_redeem_fee_basis',
    'early_redeem_fee_rate',
  ]) {
    assert.match(sources.earnApi, new RegExp(field))
  }

  assert.match(sources.modalDialog, /event\.key === 'Escape'/)
  assert.match(sources.modalDialog, /event\.key !== 'Tab'/)
  assert.match(sources.modalDialog, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(sources.modalDialog, /returnFocus\?\.focus\(\)/)
})

test('wallet ledger and message center keep error, loading, cached-data, and empty branches separate', () => {
  const ledgerBranches = [
    'v-if="error && !entries.length"',
    'v-else-if="loading"',
    'v-else-if="sortedEntries.length"',
    'v-else class="ledger-state ledger-state--empty"',
  ].map((branch) => sources.ledger.indexOf(branch))
  assert.ok(ledgerBranches.every((index) => index >= 0))
  assert.deepEqual([...ledgerBranches].sort((left, right) => left - right), ledgerBranches)
  assert.match(sources.ledger, /v-if="error && entries\.length"/)

  const messageBranches = [
    'v-if="loading && !messages.length"',
    'v-else-if="error && !messages.length"',
    'v-for="message in visibleMessages"',
    'v-else\n        :key="message.id"',
  ].map((branch) => sources.messages.indexOf(branch))
  assert.ok(messageBranches.every((index) => index >= 0))
  assert.deepEqual([...messageBranches].sort((left, right) => left - right), messageBranches)
  assert.match(sources.messages, /v-if="error && messages\.length"/)
  assert.match(sources.messages, /v-if="!loading && !error && !visibleMessages\.length"/)
})

test('compact actions expose effective 44px pointer targets', () => {
  assert.match(sources.assets, /\.assets-transfer-field input,[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(sources.help, /\.help-support-search[\s\S]*?height: 44px;/)
  assert.match(sources.help, /\.help-support-row[\s\S]*?height: 64px;[\s\S]*?min-height: 64px;/)
  assert.match(sources.newCoinRecords, /\.record-tabs button[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(sources.orders, /button\.orders-row__state::before[\s\S]*?inset: -12px -8px;/)
  assert.match(sources.messages, /\.message-filter-bar button::before[\s\S]*?inset: -9px -4px;/)
  assert.match(sources.prediction, /\.prediction-tabs button::before[\s\S]*?inset: -9px 0 -8px;/)
  assert.match(sources.prediction, /\.prediction-outcomes button::before[\s\S]*?inset: -3px 0;/)
})

test('new coin, ledger, and messages preserve their authoritative APIs', () => {
  for (const call of [
    'fetchNewCoinProjects',
    'fetchNewCoinSubscriptions',
    'fetchNewCoinDistributions',
    'fetchNewCoinPurchases',
    'fetchNewCoinUnlocks',
    'fetchWalletAccounts',
  ]) {
    assert.match(sources.newCoinRecords, new RegExp(`${call}\\(\\)`))
  }
  assert.match(sources.newCoinRecords, /await payNewCoinUnlockFee\(/)
  assert.match(sources.newCoinRecords, /await releaseNewCoinUnlock\(/)
  assert.match(sources.newCoinRecords, /\.record-tabs[\s\S]*?height: 44px;/)
  assert.match(sources.newCoinRecords, /\.record-list article[\s\S]*?min-height: 72px;/)
  assert.match(sources.newCoinRecords, /\.record-icon[\s\S]*?height: 36px;[\s\S]*?width: 36px;/)

  assert.match(sources.ledger, /fetchWalletLedger\(30, offset, filters\.find/)
  assert.match(sources.ledger, /entries\.value = reset \? rows : \[\.\.\.entries\.value, \.\.\.rows\]/)
  assert.match(sources.messages, /messages\.value = await fetchNews\(40\)/)
  assert.match(sources.messages, /hippo_mobile_message_read_ids/)
  assert.doesNotMatch(sources.messages, /@\/api\/(?:wallet|trading|user)/)
})

test('new selected-page copy is symmetric and Vue templates contain no fixed CJK text', () => {
  const vueSources = Object.entries(sources)
    .filter(([name]) => name !== 'router')
    .map(([, source]) => source)
  const keys = new Set<string>()
  for (const source of vueSources) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
    const template = source.match(/<template>([\s\S]*?)<\/template>/)?.[1] || ''
    const visibleText = template
      .replace(/\{\{[\s\S]*?\}\}/g, '')
      .replace(/<[^>]+>/g, '')
    assert.doesNotMatch(template, /[\u3400-\u9fff]/)
    assert.doesNotMatch(visibleText, /[A-Za-z]/)
    assert.doesNotMatch(template, /<svg/)
    assert.doesNotMatch(template, /\p{Extended_Pictographic}/u)
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
