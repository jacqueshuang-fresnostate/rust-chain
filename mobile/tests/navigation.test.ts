import assert from 'node:assert/strict'
import test from 'node:test'
import {
  DEFAULT_TRADE_SYMBOL,
  hasUsableRouterBack,
  normalizeRouteSymbol,
  sanitizeInternalRedirect,
  updateRouteTransition,
  routeTransitionName,
} from '../src/core/navigation.ts'

test('交易路由统一交易对格式并拒绝残缺参数', () => {
  assert.equal(normalizeRouteSymbol('eth/usdt'), 'ETH_USDT')
  assert.equal(normalizeRouteSymbol('SOL-USDC'), 'SOL_USDC')
  assert.equal(normalizeRouteSymbol('BTC'), DEFAULT_TRADE_SYMBOL)
})

test('登录后重定向仅接受应用内部路径', () => {
  assert.equal(sanitizeInternalRedirect('/assets?tab=funding'), '/assets?tab=funding')
  assert.equal(sanitizeInternalRedirect('//example.com/steal'), '/')
  assert.equal(sanitizeInternalRedirect('https://example.com/steal'), '/')
  assert.equal(sanitizeInternalRedirect(undefined, '/login'), '/login')
})

test('直开详情页没有可用历史时必须走返回兜底', () => {
  assert.equal(hasUsableRouterBack({ back: '/markets' }), true)
  assert.equal(hasUsableRouterBack({ back: '//example.com' }), false)
  assert.equal(hasUsableRouterBack({ back: null }), false)
})

test('路由层级决定前进与返回动画方向', () => {
  updateRouteTransition(2, 1)
  assert.equal(routeTransitionName.value, 'route-forward')
  updateRouteTransition(0, 2)
  assert.equal(routeTransitionName.value, 'route-back')
  updateRouteTransition(1, 1)
  assert.equal(routeTransitionName.value, 'route-fade')
})
