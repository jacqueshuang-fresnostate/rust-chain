import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { isIosBrowser, isStandaloneDisplay, resolveServiceWorkerLocation } from '../src/pwa/runtime.ts'

test('PWA runtime helpers cover iOS standalone and subpath service-worker scope', () => {
  assert.equal(isIosBrowser('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)'), true)
  assert.equal(isIosBrowser('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)', 5), true)
  assert.equal(isIosBrowser('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)', 0), false)
  assert.equal(isStandaloneDisplay(true, false), true)
  assert.equal(isStandaloneDisplay(false, true), true)
  assert.equal(isStandaloneDisplay(false, false), false)
  assert.deepEqual(resolveServiceWorkerLocation('/mobile/', 'https://example.test'), {
    scope: '/mobile/',
    scriptUrl: 'https://example.test/mobile/sw.js',
  })
})

test('PWA config keeps native builds disabled and financial traffic out of runtime cache', async () => {
  const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
  const appSource = await readFile(new URL('../src/App.vue', import.meta.url), 'utf8')
  const mainSource = await readFile(new URL('../src/main.ts', import.meta.url), 'utf8')
  const pwaSource = await readFile(new URL('../src/pwa/index.ts', import.meta.url), 'utf8')
  const pwaStatusSource = await readFile(new URL('../src/components/PwaStatus.vue', import.meta.url), 'utf8')
  const marketStoreSource = await readFile(new URL('../src/stores/market.ts', import.meta.url), 'utf8')
  const indexSource = await readFile(new URL('../index.html', import.meta.url), 'utf8')
  const viteConfig = await readFile(new URL('../vite.config.ts', import.meta.url), 'utf8')
  const tauriConfig = JSON.parse(await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'))

  assert.equal(packageJson.devDependencies['vite-plugin-pwa'], '1.3.0')
  assert.match(packageJson.scripts['build:pwa'], /vite build --mode pwa/)
  assert.match(packageJson.scripts['build:tauri'], /vite build --mode tauri/)
  assert.equal(tauriConfig.build.beforeBuildCommand, 'npm run build:tauri')

  assert.match(viteConfig, /strategies: 'generateSW'/)
  assert.match(viteConfig, /registerType: 'prompt'/)
  assert.match(viteConfig, /injectRegister: null/)
  assert.match(viteConfig, /enabled: false/)
  assert.match(viteConfig, /runtimeCaching: \[\]/)
  assert.match(viteConfig, /globIgnores: \['pwa\/\*\.png', 'manifest\.webmanifest'\]/)
  assert.match(viteConfig, /publicDir: isTauriBuild \? false : 'public'/)
  assert.match(viteConfig, /isolateTauriIndexHtml\(isTauriBuild\)/)
  assert.match(viteConfig, /mode === 'pwa'/)
  assert.match(viteConfig, /mode === 'tauri'/)
  assert.doesNotMatch(viteConfig, /BackgroundSync/)
  assert.doesNotMatch(viteConfig, /StaleWhileRevalidate|CacheFirst|NetworkFirst/)
  assert.match(mainSource, /__PWA_ENABLED__ && !isTauriRuntime\(\)/)
  assert.match(pwaSource, /!__PWA_ENABLED__ \|\| isTauriRuntime\(\)/)
  assert.match(pwaSource, /navigator\.serviceWorker\.register/)
  assert.equal((appSource.match(/<PwaStatus\s*\/>/g) || []).length, 1)
  assert.match(indexSource, /data-pwa-only/)
  assert.match(pwaStatusSource, /SAFE_PROMPT_ROUTES/)
  assert.match(pwaStatusSource, /top: calc\(env\(safe-area-inset-top, 0px\) \+ 64px\)/)
  assert.match(pwaStatusSource, /\.pwa-status\s*\{[\s\S]*pointer-events: none/)
  for (const sensitiveRoute of ['trade', 'seconds', 'withdraw', 'kyc', 'security', 'login-two-factor']) {
    assert.doesNotMatch(pwaStatusSource, new RegExp(`['"]${sensitiveRoute}['"]`))
  }
  assert.match(pwaStatusSource, /promptSafeRoute\.value && pwaState\.needRefresh/)
  assert.match(marketStoreSource, /tickers\.value = next[\s\S]*updatedAt\.value = Date\.now\(\)[\s\S]*catch/)
  assert.doesNotMatch(marketStoreSource, /finally\s*\{[^}]*updatedAt\.value/)
})

test('PWA and message-center locale contracts stay complete in Chinese and English', () => {
  const pwaKeys = [
    'installTitle',
    'installDescription',
    'installAction',
    'installing',
    'iosInstallTitle',
    'iosInstallDescription',
    'dismiss',
    'updateTitle',
    'updateDescription',
    'updateNow',
    'updateLater',
    'updating',
    'offlineTitle',
    'offlineDescription',
    'offlineReadyTitle',
    'offlineReadyDescription',
    'errorTitle',
    'registrationFailed',
    'installFailed',
    'retry',
    'retrying',
  ] as const
  const messageCenterKeys = [
    'title',
    'filterAll',
    'filterUnread',
    'summaryTotal',
    'summaryUnread',
    'markAllRead',
    'allRead',
    'empty',
    'retry',
    'categoryPlatform',
    'categoryAnnouncement',
    'latest',
  ] as const

  for (const key of pwaKeys) {
    assert.ok(zhCN.pwa[key])
    assert.ok(en.pwa[key])
  }
  for (const key of messageCenterKeys) {
    assert.ok(zhCN.messageCenter[key])
    assert.ok(en.messageCenter[key])
  }
  assert.match(zhCN.messageCenter.summaryTotal, /\{total\}/)
  assert.match(zhCN.messageCenter.summaryUnread, /\{unread\}/)
  assert.match(en.messageCenter.summaryTotal, /\{total\}/)
  assert.match(en.messageCenter.summaryUnread, /\{unread\}/)
})
