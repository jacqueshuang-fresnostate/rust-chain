import assert from 'node:assert/strict'
import test from 'node:test'
import { currentApiLocale, normalizeMobileLocale, setAppLocale } from '../src/i18n/index.ts'

test('移动端语言代码兼容系统常见地区格式', () => {
  assert.equal(normalizeMobileLocale('zh_CN'), 'zh-CN')
  assert.equal(normalizeMobileLocale('zh-HK'), 'zh-CN')
  assert.equal(normalizeMobileLocale('en-US'), 'en')
  assert.equal(normalizeMobileLocale('fr-FR'), null)
})

test('界面语言可转换为后端内容接口语言', () => {
  setAppLocale('en')
  assert.equal(currentApiLocale(), 'en-US')
  setAppLocale('zh-CN')
  assert.equal(currentApiLocale(), 'zh-CN')
})
