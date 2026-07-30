import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const baseStyles = read('../src/styles/base.css')

test('根文档抑制双向越界反馈但不锁死纵向滚动', () => {
  assert.match(
    baseStyles,
    /html,\s*body\s*\{[\s\S]*?overscroll-behavior:\s*none;[\s\S]*?\}/,
  )
  assert.doesNotMatch(baseStyles, /(?:html|body)[^{]*\{[^}]*overflow-y:\s*hidden/)
  assert.doesNotMatch(baseStyles, /(?:html|body)[^{]*\{[^}]*touch-action:\s*none/)
})

test('手机画布只裁切横向溢出，不接管文档纵向滚动', () => {
  assert.match(
    baseStyles,
    /\.app-frame\s*\{[\s\S]*?overflow-x:\s*clip;[\s\S]*?overscroll-behavior-x:\s*none;/,
  )
  assert.doesNotMatch(
    baseStyles,
    /\.app-frame\s*\{[^}]*overflow-y:\s*(?:auto|scroll|hidden)/,
  )
})
