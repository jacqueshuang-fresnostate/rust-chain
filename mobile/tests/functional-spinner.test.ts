import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const base = readFileSync(new URL('../src/styles/base.css', import.meta.url), 'utf8')
const login = readFileSync(new URL('../src/views/LoginView.vue', import.meta.url), 'utf8')

test('全局只有一个功能 spinner keyframes 且仅旋转 transform', () => {
  assert.equal((base.match(/@keyframes functional-spinner/g) || []).length, 1)
  assert.match(base, /@keyframes functional-spinner\s*\{\s*to\s*\{\s*transform: rotate\(360deg\);/)
  assert.match(base, /html body :is\([\s\S]*?\.spin\.spin\.spin,[\s\S]*?\.auth-cf-turnstile-spinner[\s\S]*?\.recharge-loading__icon[\s\S]*?\.is-spinning/)
})

test('reduced-motion 与 constrained 仍以低频 steps 提供真实加载反馈', () => {
  assert.match(base, /data-performance-tier='constrained'[\s\S]*?functional-spinner 1\.8s steps\(8, end\) infinite !important/)
  assert.match(base, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?functional-spinner 1\.8s steps\(8, end\) infinite !important/)
  assert.doesNotMatch(base, /html body #app \.app-stage \.mobile-canvas :is\([\s\S]*?\.spin/)
  assert.match(login, /auth-cf-turnstile-loading[\s\S]*?auth-cf-turnstile-spinner[\s\S]*?turnstileLoading/)
})

test('受限档只降级应用框内装饰和毛玻璃，路由层不再拦截', () => {
  assert.match(base, /data-performance-tier='constrained'[^\n]*#app \.app-frame \*/)
  assert.match(base, /backdrop-filter: none !important/)
  assert.match(base, /data-performance-tier='constrained'[\s\S]*?\.route-veil[\s\S]*?display: none !important/)
  assert.match(base, /data-performance-tier='constrained'[\s\S]*?\.kyc-country-picker-mask[\s\S]*?backdrop-filter: none !important/)
})
