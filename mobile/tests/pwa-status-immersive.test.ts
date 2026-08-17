import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('../src/components/PwaStatus.vue', import.meta.url), 'utf8')

test('PWA 状态浮层保留真实状态优先级与安全路由边界', () => {
  assert.match(source, /const SAFE_PROMPT_ROUTES = new Set\(/)
  assert.match(source, /const showUpdate = computed\(\(\) => promptSafeRoute\.value && pwaState\.needRefresh/)
  assert.match(source, /const showInstall = computed\(\(\) => promptSafeRoute\.value/)
  assert.match(source, /v-if="!pwaState\.isOnline"/)
  assert.match(source, /v-if="showUpdate"/)
  assert.match(source, /v-else-if="showInstall"/)
  assert.match(source, /v-else-if="showOfflineReady"/)
  assert.match(source, /v-else-if="showPwaError"/)

  for (const sensitiveRoute of ['trade', 'seconds', 'withdraw', 'kyc', 'security', 'login-two-factor']) {
    assert.doesNotMatch(source, new RegExp(`['"]${sensitiveRoute}['"]`))
  }

  for (const action of [
    'promptPwaInstall',
    'applyPwaUpdate',
    'dismissPwaInstall',
    'dismissPwaUpdate',
    'dismissOfflineReady',
    'retryPwaRegistration',
  ]) {
    assert.match(source, new RegExp(`\\b${action}\\b`))
  }
})

test('PWA 状态使用双层玻璃浮岛、状态环境光与明确语义', () => {
  assert.match(source, /<Transition name="pwa-status-reveal">/)
  assert.match(source, /class="pwa-status__ambient" aria-hidden="true"/)
  assert.equal((source.match(/class="pwa-status__panel"/g) || []).length, 5)
  assert.match(source, /data-tone="accent"/)
  assert.match(source, /data-tone="positive"/)
  assert.match(source, /data-tone="negative"/)
  assert.match(source, /role="alert"/)
  assert.match(source, /role="status"/)
  assert.match(source, /aria-live="polite"/)
  assert.match(source, /aria-atomic="false"/)
  assert.match(source, /aria-busy=/)

  assert.match(source, /--pwa-card-radius:\s*28px/)
  assert.match(source, /--pwa-panel-radius:\s*24px/)
  assert.match(source, /\.pwa-status__card\s*\{[\s\S]*?padding:\s*3px/)
  assert.match(source, /\.pwa-status__panel\s*\{[\s\S]*?backdrop-filter:\s*blur\(22px\) saturate\(145%\)/)
  assert.match(source, /\.pwa-status__panel\s*\{[\s\S]*?inset 0 1px 0 color-mix/)
  assert.match(source, /\.pwa-status__card::after\s*\{[\s\S]*?background-size:\s*13px 13px/)
  assert.match(source, /\.pwa-status__icon\s*\{[\s\S]*?var\(--pwa-tone\)/)
  assert.doesNotMatch(source, /#0b1811|rgba\(11,\s*24,\s*17/)
  assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
})

test('PWA 浮岛在窄屏保持非阻塞交互、44px 控件与安全入场动效', () => {
  assert.match(source, /\.pwa-status\s*\{[\s\S]*?max-width:\s*var\(--app-max-width,\s*448px\)/)
  assert.match(source, /\.pwa-status\s*\{[\s\S]*?padding:\s*10px 12px 0/)
  assert.match(source, /top:\s*calc\(env\(safe-area-inset-top,\s*0px\) \+ 64px\)/)
  assert.match(source, /\.pwa-status\s*\{[\s\S]*?pointer-events:\s*none/)
  assert.match(source, /\.pwa-status__card\s*\{[\s\S]*?pointer-events:\s*auto/)
  assert.doesNotMatch(source, /\.pwa-status\s*\{[^}]*inset:\s*0/)

  assert.match(source, /\.pwa-status__button,[\s\S]*?min-height:\s*44px/)
  assert.match(source, /\.pwa-status__dismiss\s*\{[\s\S]*?height:\s*44px[\s\S]*?width:\s*44px/)
  assert.match(source, /:focus-visible[\s\S]*?outline:\s*2px solid var\(--focus\)/)
  assert.match(source, /cubic-bezier\(\.32, \.72, 0, 1\)/)
  assert.match(source, /transform:\s*translate\(-50%, -16px\) scale\(\.965\)/)
  assert.match(source, /@media \(max-width:\s*340px\)/)
  assert.match(source, /@media \(prefers-reduced-motion:\s*reduce\)/)
  assert.match(source, /prefers-reduced-motion:[\s\S]*?animation:\s*none[\s\S]*?transition:\s*none/)
})

test('PWA 浮岛继续只使用 Lucide 图标与现有文案资源', () => {
  assert.match(source, /from 'lucide-vue-next'/)
  for (const icon of ['CircleAlert', 'CloudCheck', 'Download', 'RefreshCw', 'WifiOff', 'X']) {
    assert.match(source, new RegExp(`\\b${icon}\\b`))
  }
  assert.match(source, /t\('pwa\.statusLabel'\)/)
  assert.doesNotMatch(source, /<svg|<img|https?:\/\//)
})
