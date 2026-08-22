import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import {
  filterCountryOptions,
  normalizeCountrySearchText,
} from '../src/core/countrySearch.ts'

const registerSource = readFileSync(new URL('../src/views/RegisterView.vue', import.meta.url), 'utf8')

const countries = [
  { code: 'CN', name: 'China' },
  { code: 'CI', name: 'Côte d’Ivoire' },
  { code: 'GB', name: 'United Kingdom' },
]

const localizedLabels = new Map([
  ['CN', '中国'],
  ['CI', '科特迪瓦'],
  ['GB', '英国'],
])

test('国家搜索标准化大小写、空白、标点和重音符号', () => {
  assert.equal(normalizeCountrySearchText('  Côte-d’Ivoire  '), 'cote d ivoire')
  assert.equal(normalizeCountrySearchText(' ＣＮ '), 'cn')
  assert.equal(normalizeCountrySearchText(null), '')
})

test('国家搜索同时匹配本地化名称、后端名称和 ISO 代码', () => {
  const labelFor = (country: (typeof countries)[number]) => localizedLabels.get(country.code) || country.name

  assert.deepEqual(filterCountryOptions(countries, '中国', labelFor).map((country) => country.code), ['CN'])
  assert.deepEqual(filterCountryOptions(countries, 'china', labelFor).map((country) => country.code), ['CN'])
  assert.deepEqual(filterCountryOptions(countries, 'ci', labelFor).map((country) => country.code), ['CI'])
  assert.deepEqual(filterCountryOptions(countries, 'cote ivoire', labelFor).map((country) => country.code), ['CI'])
  assert.deepEqual(filterCountryOptions(countries, 'united king', labelFor).map((country) => country.code), ['GB'])
  assert.deepEqual(filterCountryOptions(countries, '  ', labelFor), countries)
  assert.deepEqual(filterCountryOptions(countries, '不存在', labelFor), [])
})

test('注册国家字段使用可搜索、可恢复焦点的 Teleport 弹层', () => {
  assert.match(registerSource, /useModalDialog\(countryPickerOpen, countryPickerDialog, '\[data-country-search\]'\)/)
  assert.match(registerSource, /filterCountryOptions\(countries\.value, countrySearch\.value, countryLabel\)/)
  assert.match(registerSource, /class="[^"]*country-picker-trigger[^"]*"[\s\S]*?aria-haspopup="dialog"[\s\S]*?:aria-expanded="countryPickerOpen"[\s\S]*?aria-controls="register-country-picker"/)
  assert.match(registerSource, /<Teleport to="body">[\s\S]*?id="register-country-picker"[\s\S]*?role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(registerSource, /data-country-search[\s\S]*?type="search"/)
  assert.match(registerSource, /v-for="country in filteredCountries"[\s\S]*?:aria-pressed="country\.code === countryCode"[\s\S]*?@click="selectCountry\(country\.code\)"/)
  assert.match(registerSource, /trapCountryPickerFocus\(event, closeCountryPicker\)/)
  assert.doesNotMatch(registerSource, /<select v-model="countryCode"/)
})

test('注册国家搜索文案在中英文资源中完整存在', () => {
  for (const key of [
    'countrySearchPlaceholder',
    'countrySearchLabel',
    'countryPickerTitle',
    'countryPickerClose',
    'countryNoResults',
    'countrySelectedLabel',
  ] as const) {
    assert.equal(typeof zhCN.auth[key], 'string', `zh-CN missing auth.${key}`)
    assert.equal(typeof en.auth[key], 'string', `en missing auth.${key}`)
  }
})
