import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const bottomNavSource = readFileSync(new URL('../src/components/AppBottomNav.vue', import.meta.url), 'utf8')
const pageHeaderSource = readFileSync(new URL('../src/components/PageHeader.vue', import.meta.url), 'utf8')
const routerSource = readFileSync(new URL('../src/router/index.ts', import.meta.url), 'utf8')
const baseStyles = readFileSync(new URL('../src/styles/base.css', import.meta.url), 'utf8')

test('根导航保持七个有序且独立的真实目的地', () => {
  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(bottomNavSource, /t\('trade\.spot'\)/)
  assert.match(bottomNavSource, /t\('seconds\.title'\)/)
  assert.match(bottomNavSource, /t\('trade\.contract'\)/)
  assert.match(bottomNavSource, /query:\s*\{\s*mode:\s*'contract'\s*\}/)
  assert.match(bottomNavSource, /replace\s+custom/)
  assert.match(bottomNavSource, /key:\s*'seconds'[\s\S]*?primary:\s*true/)
})

test('秒合约根路由保留旧深链，消息中心使用安全返回元数据', () => {
  assert.match(
    routerSource,
    /\{\s*path:\s*'\/seconds',\s*alias:\s*'\/products\/seconds',\s*name:\s*'seconds',[\s\S]*?meta:\s*\{\s*depth:\s*0\s*\}\s*\}/,
  )
  assert.match(
    routerSource,
    /\{\s*path:\s*'\/messages',\s*name:\s*'message-center',\s*component:\s*\(\)\s*=>\s*import\('@\/views\/MessageCenterView\.vue'\),\s*meta:\s*\{\s*depth:\s*1,\s*showBottomNav:\s*false,\s*backFallback:\s*\{\s*name:\s*'home'\s*\}\s*\}\s*\}/,
  )
})

test('导航触控和头部层级契约保持可访问且不透明', () => {
  assert.match(bottomNavSource, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(bottomNavSource, /min-height:\s*66px/)
  assert.match(baseStyles, /--bottom-nav-height:\s*84px/)
  assert.match(bottomNavSource, /flex:\s*0 0 44px/)
  assert.match(bottomNavSource, /:focus-visible \.bottom-nav__icon/)
  assert.match(baseStyles, /html\s*\{[\s\S]*scrollbar-width:\s*none/)
  assert.match(baseStyles, /html::?-webkit-scrollbar\s*\{[\s\S]*display:\s*none/)
  assert.match(pageHeaderSource, /background:\s*var\(--surface\)/)
  assert.match(pageHeaderSource, /z-index:\s*var\(--layer-sticky-header\)/)
  assert.doesNotMatch(pageHeaderSource, /backdrop-filter|transparent\)/)
})
