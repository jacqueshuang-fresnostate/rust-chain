import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const helperSource = readFileSync(new URL('../src/core/modalDialog.ts', import.meta.url), 'utf8')
const assetsSource = readFileSync(new URL('../src/views/AssetsView.vue', import.meta.url), 'utf8')
const bindingsSource = readFileSync(new URL('../src/views/AccountBindingsView.vue', import.meta.url), 'utf8')
const profileSource = readFileSync(new URL('../src/views/ProfileView.vue', import.meta.url), 'utf8')

test('共享模态层助手提供 Escape、Tab 闭环、滚动锁与焦点恢复', () => {
  assert.match(helperSource, /event\.key === 'Escape'/)
  assert.match(helperSource, /event\.key !== 'Tab'/)
  assert.match(helperSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(helperSource, /returnFocus\?\.focus\(\)/)
  assert.match(helperSource, /onBeforeUnmount/)
})

test('资产划转、账户绑定和昵称编辑统一接入共享焦点合同', () => {
  for (const source of [assetsSource, bindingsSource, profileSource]) {
    assert.match(source, /useModalDialog/)
    assert.match(source, /ref="[a-zA-Z]+Dialog"/)
    assert.match(source, /@keydown="handle[A-Za-z]+DialogKeydown"/)
    assert.match(source, /role="dialog"/)
    assert.match(source, /aria-modal="true"/)
  }
  assert.match(assetsSource, /if \(transferring\.value\) return/)
  assert.match(bindingsSource, /if \(saving\.value\.startsWith\('provider-'\)\) return/)
  assert.match(profileSource, /if \(updatingName\.value\) return/)
})
