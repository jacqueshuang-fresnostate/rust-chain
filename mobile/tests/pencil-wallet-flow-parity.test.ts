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

test('钱包流程声明当前选中浅色与深色 Pencil 画板', () => {
  const expected: Record<keyof typeof sources, string> = {
    depositAsset: 'fNXT7 n5jiPN',
    depositNetwork: 'y4ifR qKfsZ',
    depositDetail: 'w5htG TCN5A',
    withdrawAsset: 'NGBmq h0WWYC',
    withdraw: 'Qa9dW o8Wsh',
    ledger: 'y6Y7TW m25xr0',
    withdrawalRecords: 'DxqMB G3HecO',
    quickRecharge: 'CyRqi cM0eg',
  }

  for (const [name, pencilIds] of Object.entries(expected) as [keyof typeof sources, string][]) {
    const source = sources[name]
    assert.match(source, new RegExp(`data-pencil-source="${pencilIds}"`))
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
  assert.match(selectedPageCss, /html\[data-theme='dark'\] \.wallet-pencil-page\s*\{[\s\S]*?--page: #000000;[\s\S]*?background: var\(--page\);/)
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
    assert.doesNotMatch(source, /--wallet-canvas|:global\(html\[data-theme='dark'\]\)/, name)
    const scopedSource = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
    const compiled = compileStyle({
      source: scopedSource,
      filename: `${name}.vue`,
      id: `data-v-wallet-${name}`,
      scoped: true,
    })
    assert.deepEqual(compiled.errors, [], name)
    assert.match(compiled.code, /\.wallet-pencil-page\[data-v-wallet-/)
  }
})

test('钱包未登录态保留 Header 与 Body，并使用 fullPath 登录重定向的紧凑提示', () => {
  for (const source of Object.values(sources)) {
    assert.match(source, /<div class="page-content [^"]+">[\s\S]*?<LoginRequiredState/)
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
  assert.match(source, /amount\.value = String\(Math\.max\(0, available\.value - fee\.value\)\)/)
  assert.match(source, /await submitWithdrawal\(\{[\s\S]*?fundPassword: fundPassword\.value \|\| undefined,[\s\S]*?totpCode: totpCode\.value \|\| undefined,/)
  assert.match(source, /role="dialog"/)
  assert.match(source, /aria-modal="true"/)
  assert.match(source, /useModalDialog\(reviewOpen, reviewDialog, '\[data-dialog-cancel\]'\)/)
})

test('账单、提币记录和快捷充值按连续列表映射且只消费真实返回数据', () => {
  assert.match(sources.ledger, /\.ledger-filter \{[\s\S]*?min-height: 44px/)
  assert.match(sources.ledger, /\.ledger-filter button \{[\s\S]*?height: 28px/)
  assert.match(sources.ledger, /\.ledger-row \{[\s\S]*?height: 56px[\s\S]*?min-height: 56px/)
  assert.match(sources.ledger, /fetchWalletLedger\(30, offset, filters\.find/)

  assert.match(sources.withdrawalRecords, /const recordFilters: RecordFilter\[\] = \['all', 'processing', 'completed', 'failed'\]/)
  assert.match(sources.withdrawalRecords, /const filteredRecords = computed/)
  assert.match(sources.withdrawalRecords, /\.records-tabs \{[\s\S]*?min-height: 34px/)
  assert.match(sources.withdrawalRecords, /\.record-row \{[\s\S]*?min-height: 64px/)
  assert.match(sources.withdrawalRecords, /records\.value = await fetchWithdrawalRecords\(\)/)

  assert.match(sources.quickRecharge, /Promise\.all\(\[fetchQuickRechargeConfig\(\), fetchQuickRechargeOrders\(\)\]\)/)
  assert.match(sources.quickRecharge, /createQuickRechargeOrder\(numericAmount\.value, platformTarget\.value\)/)
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
  for (const source of [sources.ledger, sources.withdrawalRecords, sources.quickRecharge]) {
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
