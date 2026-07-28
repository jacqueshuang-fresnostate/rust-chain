import assert from 'node:assert/strict'
import test from 'node:test'
import {
  DEFAULT_TRADE_SYMBOL,
  ROOT_ROUTE_ORDER,
  classifyRootRouteDirection,
  hasUsableRouterBack,
  normalizeRouteSymbol,
  resolveRootRouteKey,
  routeDirection,
  sanitizeInternalRedirect,
  updateRouteTransition,
  routeTransitionName,
  routeTransitionTier,
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
  assert.equal(routeDirection.value, 'forward')
  assert.equal(routeTransitionTier.value, 'secondary')
  updateRouteTransition(0, 2)
  assert.equal(routeTransitionName.value, 'route-back')
  assert.equal(routeDirection.value, 'back')
  updateRouteTransition(1, 1)
  assert.equal(routeTransitionName.value, 'route-fade')
  assert.equal(routeDirection.value, 'still')
})

test('动效根栏目按六项原型次序产生方向且 Seconds 保持二级层级', () => {
  assert.deepEqual(
    ROOT_ROUTE_ORDER,
    ['home', 'markets', 'spot', 'contract', 'assets', 'profile'],
  )
  assert.equal(resolveRootRouteKey('trade', undefined), 'spot')
  assert.equal(resolveRootRouteKey('trade', 'contract'), 'contract')
  assert.equal(resolveRootRouteKey('seconds', undefined), null)
  assert.equal(resolveRootRouteKey('markets', undefined, 'trade'), null)
  assert.equal(resolveRootRouteKey('market-detail', undefined), null)
  assert.equal(classifyRootRouteDirection('home', 'markets'), 'forward')
  assert.equal(classifyRootRouteDirection('markets', 'home'), 'back')
  assert.equal(classifyRootRouteDirection('assets', 'assets'), 'still')
})

test('仅根栏目互切启用幕帘层级，二级进退保持克制动画', () => {
  updateRouteTransition(0, 0, 'markets', 'home', true)
  assert.equal(routeDirection.value, 'forward')
  assert.equal(routeTransitionTier.value, 'root')

  updateRouteTransition(0, 0, 'home', 'markets', true)
  assert.equal(routeDirection.value, 'back')
  assert.equal(routeTransitionTier.value, 'root')

  const secondsRoot = resolveRootRouteKey('seconds', undefined)
  updateRouteTransition(1, 0, secondsRoot, 'home', true)
  assert.equal(routeDirection.value, 'forward')
  assert.equal(routeTransitionTier.value, 'secondary')

  updateRouteTransition(0, 0, null, 'spot', true)
  assert.equal(routeDirection.value, 'forward')
  assert.equal(routeTransitionTier.value, 'secondary')

  updateRouteTransition(0, 0, 'spot', null, true)
  assert.equal(routeDirection.value, 'back')
  assert.equal(routeTransitionTier.value, 'secondary')

  updateRouteTransition(0, 1, 'home', null, true)
  assert.equal(routeDirection.value, 'back')
  assert.equal(routeTransitionTier.value, 'secondary')
})
