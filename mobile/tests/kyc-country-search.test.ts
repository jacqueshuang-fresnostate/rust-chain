import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import {
  filterCountryOptions,
  matchesCountryIdentity,
} from '../src/core/countrySearch.ts'

const source = readFileSync(new URL('../src/views/KycView.vue', import.meta.url), 'utf8')

test('KYC 只适配配置允许国家并保留配置原始 country value', () => {
  assert.match(source, /const configuredCountries = computed/)
  assert.match(source, /return configured\.map\(\(value\) => \{[\s\S]*?kycCountryOption\(value, country\)/)
  assert.match(source, /form\.value\.country = country\.value/)
  assert.match(source, /const code = country\?\.code \|\| value[\s\S]*?const name = country\?\.name \|\| value[\s\S]*?const localizedLabel/)
  assert.match(source, /searchAliases: uniqueValues\(\[value, \.\.\.localizedNames\]\)/)
  assert.match(source, /filterCountryOptions\([\s\S]*?countryOptions\.value,[\s\S]*?countrySearch\.value/)
})

test('KYC 原始配置值可通过 ISO、后端名及跨语言本地化别名解析与搜索', () => {
  const backendCountry = { code: 'CN', name: '中国' }
  const aliases = ['China', '中国']
  const configuredRawValue = 'China'
  const option = {
    ...backendCountry,
    value: configuredRawValue,
    localizedLabel: '中国',
    searchAliases: [configuredRawValue, ...aliases],
  }

  assert.equal(matchesCountryIdentity(backendCountry, 'CN', aliases), true)
  assert.equal(matchesCountryIdentity(backendCountry, 'China', aliases), true)
  assert.equal(matchesCountryIdentity(backendCountry, '中国', aliases), true)
  assert.deepEqual(filterCountryOptions([option], 'CN', (country) => country.localizedLabel), [option])
  assert.deepEqual(filterCountryOptions([option], 'China', (country) => country.localizedLabel), [option])
  assert.deepEqual(filterCountryOptions([option], '中国', (country) => country.localizedLabel), [option])
  assert.equal(option.value, configuredRawValue)
  assert.match(source, /locale\.value,[\s\S]*?'en',[\s\S]*?'zh-CN'/)
  assert.match(source, /matchesCountryIdentity\(country, value, localizedCountryNames\(country\)\)/)
})

test('KYC 状态与国家目录独立加载，目录失败不会遮蔽强一致认证状态', () => {
  assert.match(source, /Promise\.allSettled\(\[[\s\S]*?fetchKycStatus\(\),[\s\S]*?fetchCountries\(\)/)
  assert.match(source, /countriesResult\.status === 'fulfilled'[\s\S]*?kycResult\.status === 'rejected'/)
  assert.match(source, /countryDirectoryError\.value = apiErrorMessage\(countriesResult\.reason, t\('kyc\.countryLoadFailed'\)\)/)
  assert.match(source, /kyc\.value = nextKyc/)
})

test('KYC 国家底部弹窗复用焦点管理并支持搜索、Escape、遮罩与无结果', () => {
  assert.match(source, /useModalDialog\(countryPickerOpen, countryPickerDialog, '\[data-country-search\]'\)/)
  assert.match(source, /setCountryPickerReturnFocus\(countryPickerTrigger\.value\)/)
  assert.match(source, /trapCountryPickerFocus\(event, closeCountryPicker\)/)
  assert.match(source, /function isCountrySelected\(value: string\)[\s\S]*?toLowerCase\(\) === form\.value\.country\.trim\(\)\.toLowerCase\(\)/)
  assert.match(source, /:aria-pressed="isCountrySelected\(country\.value\)"/)
  assert.match(source, /@click\.self="closeCountryPicker"/)
  assert.match(source, /role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(source, /v-if="!filteredCountryOptions\.length"[\s\S]*?kyc\.countryNoResults/)
  assert.match(source, /:global\(html\[data-performance-tier='constrained'\] \.kyc-picker-mask\)/)
  assert.match(source, /\.kyc-picker-search \{[^}]*background: color-mix\(in srgb, var\(--surface-elevated\) 90%, var\(--ink\)\)/)
  assert.doesNotMatch(source, /\.kyc-picker-(?:header button|search) \{[^}]*background: var\(--surface-2\)/)
})

test('KYC 国家弹窗的受限设备样式编译后仍准确作用于遮罩', () => {
  const scopedStyle = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
  const compiled = compileStyle({
    source: scopedStyle,
    filename: 'KycView.vue',
    id: 'data-v-kyc-country-search',
    scoped: true,
  })

  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /html\[data-performance-tier=['"]?constrained['"]?\] \.kyc-picker-mask\s*\{[^}]*backdrop-filter:\s*none/)
  assert.doesNotMatch(compiled.code, /html\[data-performance-tier=['"]?constrained['"]?\]\s*\{[^}]*backdrop-filter/)
})

test('KYC 国家弹窗文案在中英文完整存在', () => {
  for (const messages of [zhCN, en]) {
    assert.ok(messages.kyc.selectCountry)
    assert.ok(messages.kyc.countryPickerTitle)
    assert.ok(messages.kyc.countryPickerClose)
    assert.ok(messages.kyc.countrySearchLabel)
    assert.ok(messages.kyc.countrySearchPlaceholder)
    assert.ok(messages.kyc.countryNoResults)
    assert.ok(messages.kyc.countryLoadFailed)
  }
})
