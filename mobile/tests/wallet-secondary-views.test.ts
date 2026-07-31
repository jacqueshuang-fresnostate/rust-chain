import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const viewFiles = {
  depositAsset: '../src/views/DepositAssetView.vue',
  depositNetwork: '../src/views/DepositNetworkView.vue',
  depositDetail: '../src/views/DepositDetailView.vue',
  withdrawAsset: '../src/views/WithdrawAssetView.vue',
  withdraw: '../src/views/WithdrawView.vue',
  withdrawalRecords: '../src/views/WithdrawalRecordsView.vue',
  walletLedger: '../src/views/WalletLedgerView.vue',
  quickRecharge: '../src/views/QuickRechargeView.vue',
} as const

const sources = Object.fromEntries(
  Object.entries(viewFiles).map(([key, path]) => [key, readFileSync(new URL(path, import.meta.url), 'utf8')]),
) as Record<keyof typeof viewFiles, string>

test('充值流程保留真实资产、网络、地址、二维码与复制合同', () => {
  assert.match(sources.depositAsset, /assets\.value = await fetchDepositAssets\(\)/)
  assert.match(sources.depositAsset, /name: 'deposit-network', params: \{ asset: asset\.symbol \}/)
  assert.match(sources.depositNetwork, /await fetchDepositAssets\(\)/)
  assert.match(sources.depositNetwork, /await fetchDepositNetworks\(props\.asset, minimum\.value\)/)
  assert.match(sources.depositNetwork, /name: 'deposit-detail', params: \{ asset: props\.asset, network: network\.network \}/)
  assert.match(sources.depositDetail, /address\.value = await createDepositAddress\(props\.asset, props\.network, minimum\)/)
  assert.match(sources.depositDetail, /toDataURL\(address\.value\.address/)
  assert.match(sources.depositDetail, /navigator\.clipboard\.writeText\(address\.value\.address\)/)
  assert.match(sources.depositDetail, /document\.execCommand\('copy'\)/)
  assert.match(sources.depositDetail, /v-if="address\.memo"/)
  assert.match(sources.depositDetail, /:fallback="\{ name: 'deposit-network', params: \{ asset \} \}"/)
})

test('提币流程保留真实资产路由、余额网络加载、校验和提交载荷', () => {
  assert.match(sources.withdrawAsset, /assets\.value = await fetchWithdrawalAssets\(\)/)
  assert.match(sources.withdrawAsset, /name: 'withdraw', params: \{ asset: asset\.symbol \}/)
  assert.match(sources.withdraw, /fetchWithdrawalAssets\(\),\s*fetchWalletAccounts\(\),\s*fetchDepositNetworks\(props\.asset\),/)
  assert.match(sources.withdraw, /amount\.value = String\(Math\.max\(0, available\.value - fee\.value\)\)/)
  assert.match(sources.withdraw, /numericAmount\.value > available\.value/)
  assert.match(sources.withdraw, /await submitWithdrawal\(\{[\s\S]*assetSymbol: asset\.value\.symbol,[\s\S]*network: selectedNetwork\.value \|\| undefined,[\s\S]*address: address\.value,[\s\S]*amount: numericAmount\.value,[\s\S]*fee: fee\.value,[\s\S]*fundPassword: fundPassword\.value \|\| undefined,[\s\S]*totpCode: totpCode\.value \|\| undefined,/)
  assert.match(sources.withdraw, /name: 'withdrawal-records'/)
  assert.match(sources.withdraw, /name: 'wallet-ledger'/)
})

test('提币记录、资金流水和快捷买币保留真实读取与支付行为', () => {
  assert.match(sources.withdrawalRecords, /records\.value = await fetchWithdrawalRecords\(\)/)
  assert.match(sources.withdrawalRecords, /return statusKeys\[status\] \? t\(statusKeys\[status\]\) : status/)
  assert.match(sources.walletLedger, /fetchWalletLedger\(30, offset, filters\.find\(\(filter\) => filter\.key === activeFilter\.value\)\?\.value\)/)
  assert.match(sources.walletLedger, /entries\.value = reset \? rows : \[\.\.\.entries\.value, \.\.\.rows\]/)
  assert.match(sources.quickRecharge, /Promise\.all\(\[fetchQuickRechargeConfig\(\), fetchQuickRechargeOrders\(\)\]\)/)
  assert.match(sources.quickRecharge, /createQuickRechargeOrder\(numericAmount\.value, platformTarget\.value\)/)
  assert.match(sources.quickRecharge, /window\.location\.assign\(submittedOrder\.value\.paymentUrl\)/)
  assert.match(sources.quickRecharge, /orders\.value = \[order, \.\.\.orders\.value\.filter/)
})

test('钱包二级页使用 HIPPO 变量、明暗主题焦点、状态和窄屏合同', () => {
  for (const source of Object.values(sources)) {
    const styles = scopedStyles(source)
    assert.match(styles, /@media \(max-width: 340px\)/)
    assert.match(styles, /env\(safe-area-inset-bottom\)/)
    assert.match(styles, /var\(--(?:surface|surface-elevated|field-surface|soft|line|ink|muted|accent|positive|negative|focus)/)
    assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(styles, /background:\s*white/i)
    assert.doesNotMatch(styles, /rgba?\(11,\s*24,\s*17/i)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
  }

  for (const source of [sources.depositAsset, sources.withdrawAsset, sources.withdraw, sources.quickRecharge]) {
    assert.match(source, /:focus-within/)
  }

  assert.match(sources.depositDetail, /height: 44px/)
  assert.match(sources.withdraw, /min-height: 44px/)
  assert.match(sources.walletLedger, /min-height: 44px/)
  assert.match(sources.quickRecharge, /min-height: 44px/)
  assert.match(sources.withdraw, /aria-invalid/)
  assert.match(sources.quickRecharge, /aria-invalid/)
  assert.match(sources.walletLedger, /:aria-pressed=/)
  assert.match(sources.quickRecharge, /:aria-pressed=/)
  assert.match(sources.withdrawalRecords, /is-positive/)
  assert.match(sources.withdrawalRecords, /is-negative/)
})

test('钱包二级页静态文案只使用现有中英文资源', () => {
  const keys = new Set<string>()
  for (const source of Object.values(sources)) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }

  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})

function scopedStyles(source: string): string {
  return source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
}

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
