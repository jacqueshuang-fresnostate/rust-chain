import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const sources = {
  depositAsset: read('../src/views/DepositAssetView.vue'),
  depositNetwork: read('../src/views/DepositNetworkView.vue'),
  depositDetail: read('../src/views/DepositDetailView.vue'),
  withdrawAsset: read('../src/views/WithdrawAssetView.vue'),
  withdraw: read('../src/views/WithdrawView.vue'),
  ledger: read('../src/views/WalletLedgerView.vue'),
  withdrawalRecords: read('../src/views/WithdrawalRecordsView.vue'),
  quickRecharge: read('../src/views/QuickRechargeView.vue'),
}
const loginRequiredSource = read('../src/components/LoginRequiredState.vue')
const pageHeaderSource = read('../src/components/PageHeader.vue')
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')
const mainSource = read('../src/main.ts')
const walletApiSource = read('../src/api/wallet.ts')
const recordsLayoutSource = read('../src/components/TransactionRecordsLayout.vue')
const recordsEmptySource = read('../src/components/TransactionRecordEmptyState.vue')

test('钱包流程声明当前选中浅色与深色 Pencil 画板', () => {
  const expected: Record<keyof typeof sources, string> = {
    depositAsset: 'fNXT7 n5jiPN',
    depositNetwork: 'y4ifR qKfsZ',
    depositDetail: 'w5htG TCN5A',
    withdrawAsset: 'NGBmq h0WWYC',
    withdraw: 'Qa9dW o8Wsh',
    ledger: 'kcP5D A85if',
    withdrawalRecords: 'DxqMB G3HecO',
    quickRecharge: 'CyRqi cM0eg',
  }

  for (const [name, pencilIds] of Object.entries(expected) as [keyof typeof sources, string][]) {
    const source = sources[name]
    assert.match(source, new RegExp(`data-pencil-source="${pencilIds}"`))
    if (name === 'ledger') {
      assert.match(source, /<TransactionRecordsLayout[\s\S]*?class="wallet-pencil-page wallet-ledger-pencil"/)
      assert.match(source, /active-tab="ledger"/)
      assert.match(source, /:back-fallback="\{ name: 'assets' \}"/)
      continue
    }
    assert.match(source, /class="page page--plain pencil-page wallet-pencil-page/)
    assert.match(source, /<PageHeader[\s\S]*?:back="true"[\s\S]*?:pencil="true"/)
    assert.match(source, /padding: 6px 20px calc\(20px \+ env\(safe-area-inset-bottom\)\)/)
    assert.doesNotMatch(source, /page--prototype-grid/)
  }

  assert.match(pageHeaderSource, /\.pencil-page-header[\s\S]*?height: 60px[\s\S]*?min-height: 60px/)
})

test('钱包白色与纯黑画布规则位于全局构建入口且 scoped 编译不再吞掉暗色选择器', () => {
  assert.match(mainSource, /import '\.\/styles\/pencil-selected-pages\.css'/)
  assert.match(selectedPageCss, /\.wallet-pencil-page\s*\{[\s\S]*?--page: #ffffff;[\s\S]*?background: var\(--page\);/)
  assert.match(selectedPageCss, /\.wallet-pencil-page\s*\{[\s\S]*?--muted: #7a8b80;[\s\S]*?--page: #ffffff;[\s\S]*?background: var\(--page\);/)
  assert.match(selectedPageCss, /html\[data-theme='dark'\] \.wallet-pencil-page\s*\{[\s\S]*?--page: #000000;[\s\S]*?--muted: #7a8b80;[\s\S]*?background: var\(--page\);/)
  assert.match(sources.ledger, /\.wallet-ledger-pencil\s*\{[\s\S]*?--wallet-record-buy: #0dbe7b;[\s\S]*?--wallet-record-canvas: #ffffff;[\s\S]*?--wallet-record-card: #ffffff;[\s\S]*?--wallet-record-chrome: #ffffff;[\s\S]*?--wallet-record-ink: #111714;[\s\S]*?--wallet-record-row-muted: #8a948f;/)
  assert.match(sources.ledger, /:global\(html\[data-theme='dark'\] \.wallet-ledger-pencil\)\s*\{[\s\S]*?--wallet-record-buy: #45efae;[\s\S]*?--wallet-record-canvas: #000000;[\s\S]*?--wallet-record-card: #000000;[\s\S]*?--wallet-record-chrome: #000000;[\s\S]*?--wallet-record-ink: #f3f7f5;[\s\S]*?--wallet-record-row-muted: #8f9b94;/)
  assert.match(selectedPageCss, /html\[data-theme='dark'\] \.wallet-pencil-page \.deposit-detail__qr\s*\{[\s\S]*?filter: invert\(1\);/)

  const globalBuildCss = compileStyle({
    source: selectedPageCss,
    filename: 'pencil-selected-pages.css',
    id: 'data-v-selected-pages',
    scoped: false,
  })
  assert.deepEqual(globalBuildCss.errors, [])
  assert.match(globalBuildCss.code, /html\[data-theme='dark'\] \.wallet-pencil-page/)
  assert.match(globalBuildCss.code, /html\[data-theme='dark'\] \.wallet-pencil-page \.deposit-detail__qr/)

  for (const [name, source] of Object.entries(sources)) {
    if (name === 'ledger') assert.doesNotMatch(source, /--wallet-canvas/, name)
    else assert.doesNotMatch(source, /--wallet-canvas|:global\(html\[data-theme='dark'\]\)/, name)
    const scopedSource = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
    const compiled = compileStyle({
      source: scopedSource,
      filename: `${name}.vue`,
      id: `data-v-wallet-${name}`,
      scoped: true,
    })
    assert.deepEqual(compiled.errors, [], name)
    if (name === 'ledger') {
      assert.match(compiled.code, /\.wallet-ledger-pencil\[data-v-wallet-/)
      assert.match(compiled.code, /html\[data-theme='dark'\] \.wallet-ledger-pencil\s*\{/)
    }
    else assert.match(compiled.code, /\.wallet-pencil-page\[data-v-wallet-/)
  }
})

test('钱包未登录态保留 Header 与 Body，并使用 fullPath 登录重定向的紧凑提示', () => {
  for (const [name, source] of Object.entries(sources)) {
    if (name === 'ledger') assert.match(source, /<div class="ledger-content">[\s\S]*?<LoginRequiredState/)
    else assert.match(source, /<div class="page-content [^"]+">[\s\S]*?<LoginRequiredState/)
    assert.match(source, /class="wallet-login-prompt"/)
    assert.match(source, /\.wallet-login-prompt \{[\s\S]*?background-image: none;[\s\S]*?min-height: 72px/)
    assert.match(source, /\.wallet-login-prompt :deep\(\.button\)[\s\S]*?min-height: 44px/)
  }

  assert.match(loginRequiredSource, /query: \{ redirect: route\.fullPath \}/)
  assert.doesNotMatch(loginRequiredSource, /router\.replace\(\{ name: 'login'/)
})

test('充币与提币选择页锁定 44px 搜索、36px 币标和 60px 真实资产行', () => {
  for (const source of [sources.depositAsset, sources.withdrawAsset]) {
    assert.match(source, /<AssetMark[\s\S]*?:size="36"/)
    assert.match(source, /\.asset-search \{[\s\S]*?height: 44px[\s\S]*?padding: 0 14px/)
    assert.match(source, /\.asset-picker button \{[\s\S]*?grid-template-columns: 36px minmax\(0, 1fr\) 16px[\s\S]*?height: 60px[\s\S]*?min-height: 60px/)
    assert.doesNotMatch(source, /asset-heading/)
  }

  assert.match(sources.depositAsset, /assets\.value = await fetchDepositAssets\(\)/)
  assert.match(sources.depositAsset, /item\.name\?\.toUpperCase\(\)\.includes\(keyword\)/)
  assert.match(sources.depositAsset, /asset\.name \|\| t\('deposit\.supported'\)/)
  assert.match(sources.withdrawAsset, /assets\.value = await fetchWithdrawalAssets\(\)/)
})

test('充币网络与地址页锁定选中摘要、56px 网络行、180px QR 和 48px 复制动作', () => {
  assert.match(sources.depositNetwork, /selectedAsset\.value = assets\.find/)
  assert.match(sources.depositNetwork, /networks\.value = await fetchDepositNetworks\(props\.asset, minimum\.value\)/)
  assert.doesNotMatch(sources.depositNetwork, /estimatedMinutes/)
  assert.match(sources.depositNetwork, /\.network-summary \{[\s\S]*?min-height: 48px/)
  assert.match(sources.depositNetwork, /\.network-list button \{[\s\S]*?height: 56px[\s\S]*?min-height: 56px/)
  assert.doesNotMatch(sources.depositNetwork, /network-note|<Info/)

  assert.match(sources.depositDetail, /address\.value = await createDepositAddress\(props\.asset, props\.network, minimum\)/)
  assert.match(sources.depositDetail, /toDataURL\(address\.value\.address/)
  assert.match(sources.depositDetail, /\.deposit-detail \{[\s\S]*?gap: 14px/)
  assert.match(sources.depositDetail, /\.deposit-detail__qr \{[\s\S]*?height: 180px[\s\S]*?width: 180px/)
  assert.match(sources.depositDetail, /\.deposit-detail__copy \{[\s\S]*?height: 48px[\s\S]*?min-height: 48px/)
  assert.match(sources.depositDetail, /navigator\.clipboard\.writeText\(address\.value\.address\)/)
})

test('提币表单保留真实网络、余额、完整字段焦点、验证和可访问复核层', () => {
  const source = sources.withdraw
  assert.match(source, /fetchWithdrawalAssets\(\),\s*fetchWalletAccounts\(\),\s*fetchDepositNetworks\(props\.asset\),/)
  assert.match(source, /\.withdraw-identity \{[\s\S]*?grid-template-columns: 34px minmax\(0, 1fr\) auto[\s\S]*?min-height: 42px/)
  assert.match(source, /\.withdraw-field \{[\s\S]*?min-height: 60px/)
  assert.match(source, /\.withdraw-field:focus-within[\s\S]*?box-shadow: 0 0 0 2px var\(--focus-ring\)/)
  assert.match(source, /\.withdraw-field input \{[\s\S]*?border: 0[\s\S]*?box-shadow: none[\s\S]*?outline: 0/)
  assert.match(source, /:aria-invalid="addressInvalid"/)
  assert.match(source, /:aria-invalid="amountInvalid"/)
  assert.match(source, /const maximum = maximumQuotedWithdrawalAmountText\([\s\S]*amount\.value = maximum === '0' \? '' : maximum/)
  assert.match(source, /const authorized = await fetchWithdrawalQuote\(\{[\s\S]*?amount: requestedAmount,/)
  assert.match(source, /await submitWithdrawal\(\{[\s\S]*?quote: quote\.value,[\s\S]*?fundPassword: fundPassword\.value \|\| undefined,[\s\S]*?totpCode: totpCode\.value \|\| undefined,/)
  assert.match(source, /role="dialog"/)
  assert.match(source, /aria-modal="true"/)
  assert.match(source, /useModalDialog\(reviewOpen, reviewDialog, '\[data-dialog-cancel\]'\)/)
})

test('交易记录使用正式通栏布局，提币记录和快捷充值继续只消费真实返回数据', () => {
  assert.match(recordsLayoutSource, /\.records-header \{[\s\S]*?grid-template-columns: 26px minmax\(0, 1fr\) 26px;[\s\S]*?height: 58px;[\s\S]*?padding: 0 16px;/)
  assert.match(recordsLayoutSource, /\.records-header h1 \{[\s\S]*?font-size: 22px;[\s\S]*?font-weight: 700;/)
  assert.match(recordsLayoutSource, /\.records-tabs \{[\s\S]*?grid-template-columns: repeat\(4, minmax\(0, 1fr\)\);[\s\S]*?height: 52px;/)
  assert.match(recordsLayoutSource, /\.records-tab\.is-active i \{[\s\S]*?var\(--records-active\)/)
  assert.match(sources.ledger, /\.ledger-filter-bar \{[\s\S]*?gap: 24px;[\s\S]*?height: 58px;[\s\S]*?padding: 0 16px;/)
  assert.match(sources.ledger, /<ListFilter :size="24"/)
  assert.match(sources.ledger, /\.ledger-filter-trigger,[\s\S]*?\.ledger-filter-more \{[\s\S]*?min-height: 44px;/)
  assert.match(sources.ledger, /<article[\s\S]*?class="ledger-row"[\s\S]*?role="listitem"/)
  assert.match(sources.ledger, /<header class="ledger-row__header">[\s\S]*?<div class="ledger-row__details">[\s\S]*?<footer class="ledger-row__footer">/)
  assert.match(sources.ledger, /\.ledger-list \{[\s\S]*?display: block;[\s\S]*?padding: 0/)
  assert.match(sources.ledger, /\.ledger-row \{[\s\S]*?align-items: stretch;[\s\S]*?background: var\(--wallet-record-card\);[\s\S]*?border-bottom: 1px solid var\(--wallet-record-row-line\);[\s\S]*?border-radius: 0;[\s\S]*?gap: 9px;[\s\S]*?min-height: 190px;[\s\S]*?padding: 12px 18px;/)
  assert.match(sources.ledger, /\.ledger-row__details \{[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  assert.match(sources.ledger, /\.ledger-row__footer \{[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  const ledgerRowRules = [...sources.ledger.matchAll(/\.ledger-row \{([^}]*)\}/g)].map((match) => match[1])
  assert.ok(ledgerRowRules.length)
  for (const rule of ledgerRowRules) {
    assert.doesNotMatch(rule, /(?:^|;)\s*(?:height|max-height)\s*:/)
  }
  assert.doesNotMatch(sources.ledger, /[^{}]*\.ledger-row[^{}]*\{[^}]*(?:-webkit-)?backdrop-filter\s*:/)
  assert.match(recordsEmptySource, /<ReceiptText :size="30"/)
  assert.match(sources.ledger, /createWalletLedgerPaginationController\(\{[\s\S]*?fetchPage: fetchWalletLedger/)
  assert.match(sources.ledger, /walletAssetSymbols\.value = result\.value\.symbols/)
  assert.match(sources.ledger, /walletAssetLogoUrls\.value = result\.value\.logoUrls/)
  assert.match(sources.ledger, /<AssetMark :symbol="entry\.symbol" :src="entryLogoUrl\(entry\)" :size="30"/)
  assert.match(sources.ledger, /v-for="entry in entries"/)
  assert.match(sources.ledger, /useModalDialog\(\s*filterSheetOpen,\s*filterDialog/)
  assert.doesNotMatch(sources.ledger, /ledger-account-filter|ledger-group__header|groupWalletLedgerEntries/)
  assert.doesNotMatch(sources.ledger, /<span class="sr-only">\{\{ entryAccessibleDetails\(entry\) \}\}<\/span>/)
  assert.doesNotMatch(sources.ledger, /grid-template-columns: repeat\(3, minmax\(0, 1fr\)\)/)

  assert.match(sources.withdrawalRecords, /const recordFilters: RecordFilter\[\] = \['all', 'processing', 'completed', 'failed'\]/)
  assert.match(sources.withdrawalRecords, /const filteredRecords = computed/)
  assert.match(sources.withdrawalRecords, /\.records-tabs \{[\s\S]*?min-height: 34px/)
  assert.match(sources.withdrawalRecords, /\.record-row \{[\s\S]*?min-height: 64px/)
  assert.match(sources.withdrawalRecords, /records\.value = await fetchWithdrawalRecords\(\)/)

  assert.match(sources.quickRecharge, /Promise\.all\(\[fetchQuickRechargeConfig\(\), fetchQuickRechargeOrders\(\)\]\)/)
  assert.match(sources.quickRecharge, /const requestAmount = amountText\.value[\s\S]*createQuickRechargeOrder\(requestAmount, platformTarget\.value\)/)
  assert.match(sources.quickRecharge, /\.recharge-intro strong \{[\s\S]*?font-size: 20px/)
  assert.match(sources.quickRecharge, /\.recharge-amount \{[\s\S]*?min-height: 64px/)
  assert.match(sources.quickRecharge, /\.recharge-submit \{[\s\S]*?height: 48px[\s\S]*?min-height: 48px/)
  assert.doesNotMatch(sources.quickRecharge, /CARD|FAST PAY|MANUAL|credit-card|user-check/)
  assert.match(walletApiSource, /name: asset\.name\?\.trim\(\) \|\| undefined/)
  assert.doesNotMatch(walletApiSource, /networkMinutes|estimatedMinutes|\|\| 'USD'|\|\| 'USDT'/)
})

test('钱包返回合同使用业务父级，动态充币详情回到当前资产网络选择', () => {
  assert.match(sources.depositAsset, /:fallback="\{ name: 'assets' \}"/)
  assert.match(sources.depositNetwork, /:fallback="\{ name: 'deposit-asset' \}"/)
  assert.match(sources.depositDetail, /:fallback="\{ name: 'deposit-network', params: \{ asset \} \}"/)
  assert.match(sources.withdrawAsset, /:fallback="\{ name: 'assets' \}"/)
  assert.match(sources.withdraw, /:fallback="\{ name: 'withdraw-asset' \}"/)
  assert.match(sources.ledger, /:back-fallback="\{ name: 'assets' \}"/)
  for (const source of [sources.withdrawalRecords, sources.quickRecharge]) {
    assert.match(source, /:fallback="\{ name: 'assets' \}"/)
  }
})

test('生产钱包页面没有复制 Pencil 演示资产、金额、网络或支付渠道', () => {
  for (const source of Object.values(sources)) {
    assert.doesNotMatch(source, /09:41|100\.00|TRC20|ERC20|BEP20|CARD|FAST PAY|MANUAL/)
    assert.doesNotMatch(source, /<svg|\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
  }
})
