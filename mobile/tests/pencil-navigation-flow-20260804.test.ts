import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createLoginRedirectTarget,
  goBackOr,
  replaceAuthStep,
  resolveRouteShellVisibility,
} from '../src/core/navigation.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const appSource = read('../src/App.vue')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const newsSource = read('../src/views/NewsView.vue')
const profileSource = read('../src/views/ProfileView.vue')
const routerSource = read('../src/router/index.ts')

test('Pencil 壳层在消息、现货、合约和秒合约路由上保持独立', () => {
  assert.deepEqual(
    resolveRouteShellVisibility('message-center', undefined, undefined, false),
    { showBottomNav: false, showRootHeader: false, showSignalField: false },
  )
  assert.deepEqual(
    resolveRouteShellVisibility('trade', undefined, undefined, true),
    { showBottomNav: true, showRootHeader: false, showSignalField: false },
  )
  assert.deepEqual(
    resolveRouteShellVisibility('trade', 'contract', undefined, true),
    { showBottomNav: false, showRootHeader: false, showSignalField: false },
  )
  assert.deepEqual(
    resolveRouteShellVisibility('seconds', undefined, undefined, false),
    { showBottomNav: false, showRootHeader: false, showSignalField: false },
  )
  assert.deepEqual(
    resolveRouteShellVisibility('markets', undefined, 'trade', true),
    { showBottomNav: false, showRootHeader: false, showSignalField: false },
  )
  assert.deepEqual(
    resolveRouteShellVisibility('home', undefined, undefined, undefined),
    { showBottomNav: true, showRootHeader: true, showSignalField: true },
  )

  assert.match(appSource, /resolveRouteShellVisibility\([\s\S]*?route\.meta\.showBottomNav/)
  assert.match(appSource, /<RootHeader v-if="showRootHeader" \/>/)
  assert.match(appSource, /<AppBottomNav v-if="showBottomNav" \/>/)
  assert.match(
    routerSource,
    /path:\s*'\/messages'[\s\S]*?meta:\s*\{\s*depth:\s*1,\s*showBottomNav:\s*false/,
  )
  assert.match(
    routerSource,
    /path:\s*'\/trade\/:symbol\?'[\s\S]*?meta:\s*\{\s*depth:\s*0,\s*showBottomNav:\s*true\s*\}/,
  )
  assert.match(
    routerSource,
    /path:\s*'\/seconds'[\s\S]*?meta:\s*\{\s*showBottomNav:\s*false,\s*depth:\s*1,\s*backFallback:\s*'\/'\s*\}/,
  )
})

test('Profile 设置按登录态分流并为登录注册保留 profile 回跳', () => {
  assert.match(
    profileSource,
    /function openSettings\(\): void \{\s*void router\.push\(\{ name: session\.isAuthenticated \? 'security' : 'language' \}\)\s*\}/,
  )
  assert.match(profileSource, /@click="openSettings"/)
  assert.match(
    profileSource,
    /name: 'login', query: \{ redirect: '\/profile' \}/,
  )
  assert.match(
    profileSource,
    /name: 'register', query: \{ redirect: '\/profile' \}/,
  )
})

test('News 从 category 深链恢复真实产品分类且普通入口默认全部', () => {
  assert.match(newsSource, /type NewsCategory = 'all' \| 'market' \| 'product' \| 'research'/)
  assert.match(newsSource, /normalizeNewsCategory\(route\.query\.category\)/)
  assert.match(newsSource, /\(\) => route\.query\.category,[\s\S]*?normalizeNewsCategory\(category\)/)
  assert.match(newsSource, /product:\s*\['product', t\('news\.product'\)\]/)
  assert.match(newsSource, /matchesCategory\(item\.category, activeCategory\.value\)/)
  assert.match(newsSource, /:\s*'all'\s*\n\}/)
})

test('根 Dock replace、认证 redirect 和 goBackOr 回退合同保持', async () => {
  assert.match(bottomNavSource, /function selectRoot\(item: RootNavigationItem, event: MouseEvent\)[\s\S]*?router\.replace\(item\.to\)/)
  assert.match(bottomNavSource, /createBottomNavSecondsTarget\(\)/)
  assert.deepEqual(createLoginRedirectTarget('/profile/security'), {
    name: 'login',
    query: { redirect: '/profile/security' },
  })
  assert.deepEqual(createLoginRedirectTarget('//outside.example'), {
    name: 'login',
    query: { redirect: '/' },
  })

  const calls: string[] = []
  const router = {
    options: { history: { state: { back: '/products' } } },
    back: () => { calls.push('back') },
    replace: async (target: unknown) => { calls.push(`replace:${JSON.stringify(target)}`) },
  }

  await goBackOr(router as never, { name: 'home' })
  assert.deepEqual(calls, ['back'])

  calls.length = 0
  router.options.history.state.back = '//outside.example'
  await goBackOr(router as never, { name: 'home' })
  assert.deepEqual(calls, ['replace:{"name":"home"}'])

  calls.length = 0
  await replaceAuthStep(router as never, createLoginRedirectTarget('/profile/security'))
  assert.deepEqual(calls, ['replace:{"name":"login","query":{"redirect":"/profile/security"}}'])
})
