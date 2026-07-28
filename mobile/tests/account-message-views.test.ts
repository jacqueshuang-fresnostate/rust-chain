import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const assetsSource = readFileSync(new URL('../src/views/AssetsView.vue', import.meta.url), 'utf8')
const profileSource = readFileSync(new URL('../src/views/ProfileView.vue', import.meta.url), 'utf8')
const securitySource = readFileSync(new URL('../src/views/SecurityView.vue', import.meta.url), 'utf8')
const messageCenterSource = readFileSync(new URL('../src/views/MessageCenterView.vue', import.meta.url), 'utf8')

test('资产页保留真实钱包、行情、资金划转与资金路由合同', () => {
  assert.match(assetsSource, /Promise\.all\(\[marketStore\.refresh\(\), fetchWalletAccounts\(\), fetchMarginWallets\(\)\]\)/)
  assert.match(assetsSource, /await transferWalletFunds\(transferAsset\.value, transferFrom\.value, to, amount\)/)
  assert.match(assetsSource, /name: 'deposit-asset'/)
  for (const routeName of ['withdraw-asset', 'wallet-ledger', 'quick-recharge']) {
    assert.match(assetsSource, new RegExp(`'${routeName}'`))
  }
  assert.match(assetsSource, /session\.logout/)
  assert.match(assetsSource, /role="dialog"/)
  assert.match(assetsSource, /aria-modal="true"/)
})

test('资料页保留资料、头像、认证状态与账户操作合同', () => {
  assert.match(profileSource, /Promise\.all\(\[fetchUserProfile\(\), fetchKycStatus\(\)\]\)/)
  assert.match(profileSource, /await updateUsername\(nameDraft\.value\)/)
  assert.match(profileSource, /await uploadUserAvatar\(file\)/)
  for (const routeName of ['kyc', 'security', 'account-bindings', 'referrals', 'language']) {
    assert.match(profileSource, new RegExp(`name: '${routeName}'`))
  }
  assert.match(profileSource, /session\.logout\(\)/)
  assert.match(profileSource, /router\.replace\('\/'\)/)
  assert.match(profileSource, /role="dialog"/)
  assert.match(profileSource, /aria-modal="true"/)
})

test('安全页保留登录密码、资金密码与双重验证完整 API 流程', () => {
  for (const call of [
    'changeLoginPassword',
    'setFundPassword',
    'changeFundPassword',
    'sendFundPasswordResetCode',
    'resetFundPassword',
    'fetchTwoFactorStatus',
    'setupTwoFactor',
    'confirmTwoFactor',
    'updateLoginTwoFactor',
    'sendUserTwoFactorResetCode',
    'resetUserTwoFactor',
  ]) {
    assert.match(securitySource, new RegExp(`\\b${call}\\(`))
  }
  assert.match(securitySource, /target\.checked = !enabled/)
  assert.match(securitySource, /session\.sync\(\)/)
})

test('消息中心只展示真实公告、保存本机已读 ID 并进入公告详情', () => {
  assert.match(messageCenterSource, /await fetchNews\(40\)/)
  assert.match(messageCenterSource, /hippo_mobile_message_read_ids/)
  assert.match(messageCenterSource, /globalThis\.localStorage\?\.getItem\(READ_IDS_STORAGE_KEY\)/)
  assert.match(messageCenterSource, /globalThis\.localStorage\?\.setItem\(READ_IDS_STORAGE_KEY, JSON\.stringify\(values\)\)/)
  assert.match(messageCenterSource, /readIds\.value = new Set\(\[\.\.\.readIds\.value, id\]\)/)
  assert.match(messageCenterSource, /router\.push\(\{ name: 'news-detail', params: \{ id: String\(message\.id\) \} \}\)/)
  assert.doesNotMatch(messageCenterSource, /@\/api\/(?:user|wallet|trading|orders)/)
  assert.doesNotMatch(messageCenterSource, /messages\.value\s*=\s*\[/)
})

test('账户与消息视图满足主题、触控、窄屏和 Lucide 契约', () => {
  for (const source of [assetsSource, profileSource, securitySource, messageCenterSource]) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /min-height: (?:4[4-9]|[5-9]\d)px/)
    assert.match(source, /env\(safe-area-inset-bottom\)/)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(source, /background:\s*(?:white|rgb\()/i)
  }
  assert.match(assetsSource, /:focus-within/)
  assert.match(securitySource, /:focus-within/)
  assert.match(securitySource, /\.switch input:focus-visible \+ i/)
  assert.match(messageCenterSource, /grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/)
  assert.match(messageCenterSource, /aria-pressed/)
})

test('四个视图引用的固定文案键在中英文资源中均存在', () => {
  const keys = new Set<string>()
  for (const source of [assetsSource, profileSource, securitySource, messageCenterSource]) {
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
