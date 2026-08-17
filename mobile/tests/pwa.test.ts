import assert from 'node:assert/strict'
import { access, readdir, readFile } from 'node:fs/promises'
import { extname } from 'node:path'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { isIosBrowser, isStandaloneDisplay, resolveServiceWorkerLocation } from '../src/pwa/runtime.ts'

const dependencySourceExtensions = new Set(['.css', '.html', '.js', '.json', '.mjs', '.ts', '.vue'])

async function collectDependencySources(directory: URL): Promise<URL[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = await Promise.all(entries.map(async (entry) => {
    const location = new URL(entry.name + (entry.isDirectory() ? '/' : ''), directory)
    if (entry.isDirectory()) return collectDependencySources(location)
    return dependencySourceExtensions.has(extname(entry.name)) ? [location] : []
  }))
  return files.flat()
}

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
  assert.match(viteConfig, /navigateFallbackDenylist:\s*\[[\s\S]*?\/\\\/api\(\?:\\\/\|\$\)\//)
  assert.match(viteConfig, /navigateFallbackDenylist:\s*\[[\s\S]*?\/\\\/ws\(\?:\\\/\|\$\)\//)
  assert.match(viteConfig, /navigateFallbackDenylist:\s*\[[\s\S]*?\/\\\/health\(\?:\\\/\|\$\)\//)
  assert.match(viteConfig, /navigateFallbackDenylist:\s*\[[\s\S]*?\/\\\/downloads\?\(\?:\\\/\|\$\)\//)
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
  assert.match(marketStoreSource, /tickers\.value = mergeMarketTickerSnapshots\(tickers\.value, next\)[\s\S]*updatedAt\.value = Date\.now\(\)[\s\S]*catch/)
  assert.doesNotMatch(marketStoreSource, /finally\s*\{[^}]*updatedAt\.value/)
})

test('production source and tests do not depend on the ignored prototype workspace', async () => {
  const ignoredPrototypeDirectory = ['sites', 'prototype'].join('-')
  const files = (await Promise.all([
    collectDependencySources(new URL('../src/', import.meta.url)),
    collectDependencySources(new URL('./', import.meta.url)),
  ])).flat()
  const violations: string[] = []

  for (const file of files) {
    if ((await readFile(file, 'utf8')).includes(ignoredPrototypeDirectory)) {
      violations.push(file.pathname)
    }
  }

  assert.deepEqual(violations, [])
})

test('copied prototype CSS resolves production-owned font and image paths', async () => {
  const stylesheets = [
    new URL('../src/styles/prototype-base.css', import.meta.url),
    new URL('../src/styles/prototype-parity.css', import.meta.url),
  ]

  for (const stylesheet of stylesheets) {
    const source = await readFile(stylesheet, 'utf8')
    const assetPaths = [...source.matchAll(/url\((?:'|")?([^'")]+)(?:'|")?\)/g)]
      .map((match) => match[1])
      .filter((path) => !path.startsWith('data:'))

    assert.ok(assetPaths.length > 0, `${stylesheet.pathname} should reference copied assets`)
    for (const assetPath of assetPaths) {
      assert.ok(!assetPath.startsWith('/'), `${assetPath} must not depend on a deployment-root file`)
      await access(new URL(assetPath, stylesheet))
    }
  }
})

test('PWA and message-center locale contracts stay complete in Chinese and English', () => {
  const pwaKeys = [
    'statusLabel',
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
    'summaryTotalLabel',
    'summaryUnreadLabel',
    'summarySource',
    'markAllRead',
    'allRead',
    'empty',
    'retry',
    'categoryLabel',
    'categoryPlatform',
    'categoryAccount',
    'categoryFunds',
    'categoryTrade',
    'categoryAnnouncement',
    'sourceContext',
    'categoryEmpty',
    'categoryEmptyDescription',
    'unreadEmptyDescription',
    'announcementEmptyDescription',
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
