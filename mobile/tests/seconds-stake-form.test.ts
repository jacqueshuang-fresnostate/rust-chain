import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createSecondsFinancialPresentation,
  validateSecondsStake,
} from '../src/core/secondsFinancial.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

function presentation(language: 'en' | 'zh-CN' = 'zh-CN') {
  const messages = language === 'en' ? en : zhCN
  return createSecondsFinancialPresentation({
    locale: () => language,
    exactByOrderId: new Map(),
    normalizeSymbol: (symbol) => symbol,
    liveTickerFor: () => undefined,
    marketTickerFor: () => undefined,
    selectedSymbol: () => '',
    selectedCandleClose: () => undefined,
    translate: (key, params = {}) => {
      const name = key.replace('seconds.', '') as keyof typeof messages.seconds
      return messages.seconds[name].replace(/\{(\w+)\}/g, (_, field: string) => params[field] || '')
    },
  })
}

test('Seconds limits expose both bounds of the selected cycle in Chinese and English', () => {
  const first = { minStake: 500, minStakeText: '500', maxStake: 1000, maxStakeText: '1000' }
  const second = { minStake: 10, minStakeText: '10', maxStake: 2500, maxStakeText: '2500' }
  const chinese = presentation()
  assert.equal(chinese.formatCycleLimit(first, 'USDT'), '最小投注 500 USDT · 最大投注 1,000 USDT')
  assert.equal(chinese.formatCycleLimit(second, 'USDT'), '最小投注 10 USDT · 最大投注 2,500 USDT')
  assert.equal(presentation('en').formatCycleLimit(first, 'USDT'), 'Min stake 500 USDT · Max stake 1,000 USDT')
})

test('Seconds nullable maximum is unlimited while malformed or numeric-only bounds stay unavailable', () => {
  const view = presentation()
  for (const maximum of [{ maxStakeText: null }, { maxStake: null }, {}, { maxStakeText: null, maxStake: 1 }]) {
    const cycle = { minStake: 10, minStakeText: '10', ...maximum }
    assert.equal(view.cycleHasMaximum(cycle), false)
    assert.equal(view.hasExactStakeRange(cycle), true)
    assert.equal(view.formatCycleLimit(cycle, 'USDT'), '最小投注 10 USDT · 最大投注不限')
    assert.equal(presentation('en').formatCycleLimit(cycle, 'USDT'), 'Min stake 10 USDT · No maximum stake')
    assert.equal(validateSecondsStake('1500', {
      minimum: view.cycleMinimum(cycle),
      maximum: view.cycleHasMaximum(cycle) ? view.cycleMaximum(cycle) : undefined,
      available: '2000',
    }).isValid, true)
  }
  for (const maximum of [{ maxStakeText: '' }, { maxStakeText: 'invalid' }, { maxStakeText: '0' }, { maxStakeText: '-1' }, { maxStake: 100 }]) {
    const cycle = { minStake: 10, minStakeText: '10', ...maximum }
    assert.equal(view.cycleHasMaximum(cycle), true)
    assert.equal(view.hasExactStakeRange(cycle), false)
    assert.equal(view.formatCycleLimit(cycle, 'USDT'), '--')
  }
  assert.equal(view.formatCycleLimit(undefined, 'USDT'), '--')
  assert.equal(view.formatCycleLimit({ minStake: 10, minStakeText: null, maxStakeText: null }, 'USDT'), '--')
})

test('Seconds available balance preserves exact zero and never fabricates missing or numeric-only wallet funds', () => {
  const view = presentation()
  assert.equal(view.formatValue(view.walletAvailable({ available: 999, availableText: '0' })), '0')
  assert.equal(view.formatValue(view.walletAvailable(undefined)), '--')
  assert.equal(view.formatValue(view.walletAvailable({ available: 999 })), '--')
  assert.equal(view.formatValue(view.walletAvailable({ available: 999, availableText: null })), '--')
  assert.equal(view.formatValue(view.walletAvailable({ available: 999, availableText: '1234.56' })), '1,234.56')
  assert.equal(view.walletAvailable({ available: 999, availableText: '9007199254740993.000000000000000001' }), '9007199254740993.000000000000000001')
  assert.equal(validateSecondsStake('1', { minimum: '1', available: '0' }).isValid, false)
  assert.equal(validateSecondsStake('', { minimum: '500', available: '1000' }).isValid, false)
})

test('Seconds view leaves stake drafts user-owned and renders a visible asset-matched balance', () => {
  const source = readFileSync(new URL('../src/views/SecondsView.vue', import.meta.url), 'utf8')
  assert.match(source, /const amount = ref\(''\)/)
  assert.doesNotMatch(source, /amount\.value\s*=\s*(?:cycleMin|['"]500['"])/)
  const selectCycle = source.slice(source.indexOf('function selectCycle('), source.indexOf('function setDirection('))
  assert.match(selectCycle, /selectedCycleId\.value = cycleId/)
  assert.doesNotMatch(selectCycle, /amount\.value\s*=/)
  const selectProduct = source.slice(source.indexOf('function selectProduct('), source.indexOf('function choosePairProduct('))
  assert.match(selectProduct, /amount\.value = ''/)
  assert.match(source, /accounts\.value\.find\(\(item\) => item\.assetId === selected\.value\?\.stakeAssetId\)/)
  const balance = source.match(/<div id="seconds-balance-hint"[\s\S]*?<\/div>/)?.[0] || ''
  assert.match(balance, /class="seconds-balance-hint"/)
  assert.doesNotMatch(balance, /sr-only|hidden/)
  assert.match(balance, /aria-live="polite"/)
  assert.match(balance, /!session\.isAuthenticated/)
  assert.match(balance, /v-else-if="loading"/)
  assert.match(balance, /moneyText\(availableStakeBalance\)/)
  assert.match(balance, /selected\?\.stakeAssetSymbol/)
})
