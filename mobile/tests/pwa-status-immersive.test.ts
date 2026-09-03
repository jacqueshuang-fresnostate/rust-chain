import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const source = readFileSync(new URL('../src/components/PwaStatus.vue', import.meta.url), 'utf8')
const baseStyleSource = readFileSync(new URL('../src/styles/base.css', import.meta.url), 'utf8')
const modalHelperSource = readFileSync(new URL('../src/core/modalDialog.ts', import.meta.url), 'utf8')
const templateSource = source.match(/<template>([\s\S]*?)<\/template>/)?.[1] || ''
const styleSource = source.match(/<style\s+scoped>([\s\S]*?)<\/style>/)?.[1] || ''
const installTemplate = templateSource.slice(templateSource.indexOf('<Teleport to="body">'))

function cssBlockIn(css: string, selector: string): string {
  const start = css.indexOf(`${selector} {`)
  assert.notEqual(start, -1, `missing CSS rule ${selector}`)
  const end = css.indexOf('\n}', start)
  assert.notEqual(end, -1, `unterminated CSS rule ${selector}`)
  return css.slice(start, end + 2)
}

function cssBlock(selector: string): string {
  return cssBlockIn(styleSource, selector)
}

function lastCssBlock(selector: string): string {
  const start = styleSource.lastIndexOf(`${selector} {`)
  assert.notEqual(start, -1, `missing CSS rule ${selector}`)
  const end = styleSource.indexOf('\n}', start)
  assert.notEqual(end, -1, `unterminated CSS rule ${selector}`)
  return styleSource.slice(start, end + 2)
}

function assertOrdered(haystack: string, needles: string[]): void {
  let previous = -1
  for (const needle of needles) {
    const current = haystack.indexOf(needle)
    assert.ok(current > previous, `${needle} must follow the previous Pencil node`)
    previous = current
  }
}

function cssLength(rule: string, property: string): number {
  const value = rule.match(new RegExp(`(?:^|\\n)\\s*${property}:\\s*(-?[\\d.]+)px;`))?.[1]
  assert.ok(value, `${property} must be an explicit pixel length`)
  return Number(value)
}

test('PWA 真实状态保留安全路由、离线并列与安装失败可见优先级', () => {
  assert.match(source, /const SAFE_PROMPT_ROUTES = new Set\(/)
  assert.match(source, /const showUpdate = computed\(\(\) => promptSafeRoute\.value && pwaState\.needRefresh/)
  assert.match(source, /const showInstall = computed\(\(\) => \([\s\S]*?!pwaState\.installError/)
  assert.match(source, /const showInstallDialog = computed\(\(\) => \([\s\S]*?pwaState\.isOnline[\s\S]*?primaryState\.value === 'install'/)
  assert.match(source, /const showStatusIsland = computed\(\(\) => pwaState\.enabled && \([\s\S]*?!pwaState\.isOnline/)

  assertOrdered(source, [
    'if (showUpdate.value)',
    'if (showPwaError.value && pwaState.installError)',
    'if (showInstall.value)',
    'if (showOfflineReady.value)',
    'if (showPwaError.value)',
  ])
  assert.match(templateSource, /v-if="!pwaState\.isOnline"/)
  assert.match(templateSource, /v-if="primaryState === 'update'"/)
  assert.match(templateSource, /v-else-if="primaryState === 'ready'"/)
  assert.match(templateSource, /v-else-if="primaryState === 'error'"/)
  assert.equal((templateSource.match(/class="pwa-status__panel"/g) || []).length, 4)
  assert.doesNotMatch(templateSource, /pwa-status__card--install/)

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
  assert.match(source, /watch\(showInstallDialog,[\s\S]*?markPwaInstallOfferShown\(\)/)
})

test('非安装态继续使用原双层玻璃浮岛、语义色和非阻塞指针边界', () => {
  assert.match(templateSource, /<Transition name="pwa-status-reveal">/)
  assert.match(templateSource, /class="pwa-status__ambient" aria-hidden="true"/)
  assert.match(templateSource, /data-tone="accent"/)
  assert.match(templateSource, /data-tone="positive"/)
  assert.match(templateSource, /data-tone="negative"/)
  assert.match(templateSource, /role="alert"/)
  assert.match(templateSource, /role="status"/)
  assert.match(templateSource, /aria-live="polite"/)
  assert.match(templateSource, /aria-atomic="false"/)
  assert.match(templateSource, /aria-busy=/)

  assert.match(cssBlock('.pwa-status'), /max-width:\s*var\(--app-max-width, 448px\)/)
  assert.match(cssBlock('.pwa-status'), /pointer-events:\s*none/)
  assert.match(cssBlock('.pwa-status'), /top:\s*calc\(env\(safe-area-inset-top, 0px\) \+ 64px\)/)
  assert.match(cssBlock('.pwa-status__card'), /border-radius:\s*var\(--pwa-card-radius\)/)
  assert.match(cssBlock('.pwa-status__card'), /pointer-events:\s*auto/)
  assert.match(cssBlock('.pwa-status__card'), /padding:\s*3px/)
  assert.match(cssBlock('.pwa-status__panel'), /backdrop-filter:\s*blur\(22px\) saturate\(145%\)/)
  assert.match(cssBlock('.pwa-status__panel'), /inset 0 1px 0 color-mix/)
  assert.doesNotMatch(source, /#0b1811|rgba\(11,\s*24,\s*17/)
})

test('Pencil NROQD/Tcgl6 安装弹窗保留全部指定结构、HIPPO 本地品牌图与 Lucide 图标', () => {
  assert.match(source, /import brandLogo from '@\/assets\/brand\/hippo-logo-landscape\.png'/)
  assert.match(source, /from 'lucide-vue-next'/)
  assert.match(installTemplate, /data-pencil-source="NROQD FwXCx Tcgl6 V04kP"/)
  assertOrdered(installTemplate, [
    'class="pwa-install__grabber"',
    'class="pwa-install__header"',
    'class="pwa-install__app-icon"',
    'class="pwa-install__heading"',
    'class="pwa-install__close"',
    'class="pwa-install__description"',
    'class="pwa-install__benefits"',
    'class="pwa-install__hint"',
    'class="pwa-install__primary"',
    'class="pwa-install__later"',
  ])
  assert.equal((installTemplate.match(/class="pwa-install__benefit"/g) || []).length, 3)
  assert.match(installTemplate, /<img :src="brandLogo" alt="" \/>/)
  for (const icon of ['Zap', 'Maximize', 'BellRing', 'Info', 'Download', 'X']) {
    assert.match(installTemplate, new RegExp(`<${icon}\\b`))
  }
  assert.doesNotMatch(installTemplate, /<svg\b|https?:\/\//)
  assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
})

test('390px Pencil 基准锁定 540px Sheet 及 82/44/146/42/54/38 关键几何', () => {
  const overlay = cssBlock('.pwa-install')
  const sheet = cssBlock('.pwa-install__sheet')
  assert.match(overlay, /height:\s*100dvh/)
  assert.match(overlay, /max-width:\s*var\(--app-max-width, 448px\)/)
  assert.match(overlay, /width:\s*100%/)
  assert.match(sheet, /border-radius:\s*26px 26px 0 0/)
  assert.match(sheet, /gap:\s*14px/)
  assert.match(sheet, /grid-template-rows:\s*82px 44px 146px 42px 54px 38px/)
  assert.match(sheet, /height:\s*min\(540px, 100dvh\)/)
  assert.match(sheet, /padding:\s*12px 20px max\(22px, env\(safe-area-inset-bottom, 0px\)\)/)

  assert.match(cssBlock('.pwa-install__grabber'), /height:\s*4px[\s\S]*?top:\s*6px[\s\S]*?width:\s*42px/)
  assert.match(cssBlock('.pwa-install__header'), /height:\s*82px/)
  assert.match(cssBlock('.pwa-install__app-icon'), /border-radius:\s*18px[\s\S]*?height:\s*64px[\s\S]*?top:\s*14px[\s\S]*?width:\s*64px/)
  assert.match(cssBlock('.pwa-install__app-icon img'), /height:\s*32px[\s\S]*?width:\s*50px/)
  assert.match(cssBlock('.pwa-install__heading'), /gap:\s*4px[\s\S]*?height:\s*55px[\s\S]*?left:\s*78px[\s\S]*?right:\s*50px[\s\S]*?top:\s*18\.5px/)
  assert.match(cssBlock('.pwa-install__heading h2'), /font-size:\s*22px[\s\S]*?font-weight:\s*700[\s\S]*?letter-spacing:\s*0[\s\S]*?line-height:\s*32px/)
  assert.doesNotMatch(cssBlock('.pwa-install__heading h2'), /letter-spacing:\s*-/)
  assert.match(cssBlock('.pwa-install__close'), /height:\s*44px[\s\S]*?right:\s*-4px[\s\S]*?top:\s*24px[\s\S]*?width:\s*44px/)
  assert.match(cssBlock('.pwa-install__close-face'), /border-radius:\s*18px[\s\S]*?height:\s*36px[\s\S]*?width:\s*36px/)
  assert.match(cssBlock('.pwa-install__description'), /font-size:\s*14px[\s\S]*?font-weight:\s*500[\s\S]*?height:\s*44px[\s\S]*?line-height:\s*1\.55/)
  assert.match(cssBlock('.pwa-install__benefits'), /border-radius:\s*16px[\s\S]*?grid-template-rows:\s*repeat\(3, 43px\)[\s\S]*?height:\s*146px[\s\S]*?padding:\s*8px 14px/)
  assert.match(cssBlock('.pwa-install__benefit-icon'), /border-radius:\s*9px[\s\S]*?height:\s*30px[\s\S]*?top:\s*6\.5px[\s\S]*?width:\s*30px/)
  assert.match(cssBlock('.pwa-install__benefit-copy'), /grid-template-rows:\s*20px 17px[\s\S]*?height:\s*38px[\s\S]*?left:\s*42px[\s\S]*?top:\s*2\.5px/)
  assert.match(cssBlock('.pwa-install__hint'), /border-radius:\s*12px[\s\S]*?gap:\s*8px[\s\S]*?height:\s*42px[\s\S]*?padding:\s*0 12px/)
  assert.match(cssBlock('.pwa-install__primary'), /border-radius:\s*16px[\s\S]*?gap:\s*8px[\s\S]*?height:\s*54px/)
  assert.match(installTemplate, /<X :size="19"/)
  assert.match(installTemplate, /<Download :size="19"/)

  assert.match(cssBlock('.pwa-install__sheet'), /box-shadow:\s*0 -8px 28px #00000024/)
  assert.match(cssBlock('.pwa-install__primary'), /box-shadow:\s*0 6px 16px #18d38d2e/)

  const rowHeights = [82, 44, 146, 42, 54, 38]
  const rowY = rowHeights.map((_, index) => 12 + rowHeights.slice(0, index).reduce((sum, value) => sum + value, 0) + index * 14)
  assert.deepEqual(rowY, [12, 108, 166, 326, 382, 450])
  assert.equal(390 - 40, 350)
  assert.equal(350 - 28, 322)
  assert.equal(350 - 78 - 50, 222)
})

test('Pencil 浅深色板在 Teleport 后保留精确色值与完整深色后代选择器', () => {
  const light = cssBlock('.pwa-install')
  for (const [token, color] of [
    ['overlay', '#07110c80'],
    ['sheet', '#ffffff'],
    ['grabber', '#cdd7d1'],
    ['app-icon', '#e3faef'],
    ['app-icon-line', '#bcebd6'],
    ['title', '#102018'],
    ['muted', '#64736b'],
    ['accent', '#18d38d'],
    ['close', '#eff8f3'],
    ['benefits', '#eff8f3'],
    ['benefit-icon', '#d8f7e9'],
    ['hint', '#f7faf8'],
    ['hint-line', '#dce9e2'],
    ['primary-text', '#082a1d'],
  ]) {
    assert.match(light, new RegExp(`--pwa-install-${token}:\\s*${color}`))
  }

  const compiled = compileStyle({
    source: styleSource,
    filename: 'PwaStatus.vue',
    id: 'data-v-pwa-status',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /html\[data-theme=['"]dark['"]\]\s+\.pwa-install\s*\{[\s\S]*?--pwa-install-overlay:\s*#000000b8/)
  for (const color of [
    '#101a15',
    '#526159',
    '#183d2f',
    '#2e7057',
    '#f4faf7',
    '#9ba8a1',
    '#18261f',
    '#214235',
    '#151f1a',
    '#293b32',
  ]) {
    assert.match(compiled.code, new RegExp(color, 'i'))
  }
  assert.doesNotMatch(compiled.code, /html\[data-theme=['"]dark['"]\]\s*\{[^}]*--pwa-install-/)
})

test('稍后按钮的最终级联尺寸覆盖全局 44px min-height 而保留 44px 命中区', () => {
  const globalButton = cssBlockIn(baseStyleSource, 'button')
  const later = lastCssBlock('.pwa-install__later')
  assert.equal(cssLength(globalButton, 'min-height'), 44, '回归必须覆盖真实全局冲突')

  const declaredHeight = cssLength(later, 'height')
  const cascadedMinHeight = cssLength(later, 'min-height')
  assert.equal(Math.max(declaredHeight, cascadedMinHeight), 38)
  assert.match(cssBlock('.pwa-install__later::before'), /inset:\s*-3px 0/)
  assert.equal(declaredHeight + 3 + 3, 44)

  const compiled = compileStyle({
    source: styleSource,
    filename: 'PwaStatus.vue',
    id: 'data-v-pwa-status',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  assert.match(
    compiled.code,
    /\.pwa-install__later\[data-v-pwa-status\]\s*\{[\s\S]*?height:\s*38px;[\s\S]*?min-height:\s*38px;/,
  )
})

test('安装 Sheet 复用共享模态生命周期并覆盖关闭、遮罩、Escape、Tab、滚动锁与焦点恢复', () => {
  assert.match(source, /import \{ useModalDialog \} from '@\/core\/modalDialog'/)
  assert.match(source, /useModalDialog\([\s\S]*?showInstallDialog,[\s\S]*?installDialog,[\s\S]*?'\[data-pwa-install-close\]'/)
  assert.match(installTemplate, /<Teleport to="body">/)
  assert.match(installTemplate, /role="dialog"/)
  assert.match(installTemplate, /aria-modal="true"/)
  assert.match(installTemplate, /aria-labelledby="pwa-install-title"/)
  assert.match(installTemplate, /aria-describedby="pwa-install-description pwa-install-hint"/)
  assert.match(installTemplate, /:aria-busy="installing"/)
  assert.match(installTemplate, /@keydown="handleInstallDialogKeydown"/)
  assert.match(installTemplate, /@click\.self="closeInstallDialog"/)
  assert.equal((installTemplate.match(/@click="closeInstallDialog"/g) || []).length, 2)
  assert.match(source, /trapInstallFocus\(event, closeInstallDialog\)/)
  assert.match(source, /function closeInstallDialog\(\): void \{[\s\S]*?if \(installing\.value\) return[\s\S]*?dismissPwaInstall\(\)/)
  assert.match(modalHelperSource, /event\.key === 'Escape'/)
  assert.match(modalHelperSource, /event\.key !== 'Tab'/)
  assert.match(modalHelperSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(modalHelperSource, /returnFocus\?\.focus\(\)/)

  assert.match(cssBlock('.pwa-install__close'), /height:\s*44px[\s\S]*?width:\s*44px/)
  assert.match(cssBlock('.pwa-install__primary'), /height:\s*54px/)
  assert.match(styleSource, /\.pwa-install__close:focus-visible,[\s\S]*?outline:\s*2px solid var\(--pwa-install-accent\)/)
})

test('Android/Chromium 主操作保留原生安装，iOS 主操作聚焦始终可读的手动提示', () => {
  assert.match(source, /if \(installing\.value\) return/)
  assert.match(source, /if \(pwaState\.iosInstallAvailable && !pwaState\.installAvailable\) \{[\s\S]*?installHint\.value\?\.focus\(\)[\s\S]*?return/)
  assert.match(source, /if \(!pwaState\.installAvailable\) return[\s\S]*?await promptPwaInstall\(\)/)
  assert.match(installTemplate, /id="pwa-install-hint"[\s\S]*?ref="installHint"[\s\S]*?role="note"[\s\S]*?tabindex="-1"/)
  assert.match(installTemplate, /\{\{ t\('pwa\.iosInstallDescription'\) \}\}/)
  assert.match(cssBlock('.pwa-install__hint'), /height:\s*42px/)
  assert.match(styleSource, /\.pwa-install__hint:focus\s*\{[\s\S]*?outline:/)
  assert.match(installTemplate, /class="pwa-install__primary"[\s\S]*?:disabled="installing"[\s\S]*?@click="install"/)
})

test('320–448px 响应式与短屏可滚动，出入场只动 opacity/transform 且尊重低动态', () => {
  assert.match(cssBlock('.pwa-install'), /max-width:\s*var\(--app-max-width, 448px\)[\s\S]*?overflow:\s*hidden[\s\S]*?width:\s*100%/)
  assert.match(cssBlock('.pwa-install__sheet'), /height:\s*min\(540px, 100dvh\)[\s\S]*?max-height:\s*100dvh/)
  assert.match(cssBlock('.pwa-install__sheet'), /overflow-x:\s*hidden[\s\S]*?overflow-y:\s*auto[\s\S]*?overscroll-behavior:\s*contain/)
  assert.match(styleSource, /@media \(max-width: 340px\) \{[\s\S]*?\.pwa-install__sheet\s*\{[\s\S]*?padding-inline:\s*16px/)
  assert.doesNotMatch(cssBlock('.pwa-install'), /width:\s*(?:320|390|448)px/)
  assert.doesNotMatch(cssBlock('.pwa-install__sheet'), /width:\s*(?:320|390|448)px/)

  const motionStart = styleSource.indexOf('.pwa-install-modal-enter-active')
  const responsiveStart = styleSource.indexOf('@media (max-width: 340px)', motionStart)
  const motion = styleSource.slice(motionStart, responsiveStart)
  assert.match(motion, /transition:\s*opacity/)
  assert.match(motion, /transition:\s*transform/)
  assert.doesNotMatch(motion, /transition:[^;]*(?:height|top|bottom|box-shadow|filter)/)
  assert.match(styleSource, /@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\.pwa-install-modal-enter-active,[\s\S]*?transition:\s*none/)
  assert.match(styleSource, /prefers-reduced-motion:[\s\S]*?\.pwa-install-modal-enter-from \.pwa-install__sheet,[\s\S]*?transform:\s*none/)
})

test('安装弹窗中英文案完整对称，中文与 Pencil 真值逐项一致', () => {
  const keys = [
    'installTitle',
    'installSubtitle',
    'installDescription',
    'installBenefitsLabel',
    'installFastTitle',
    'installFastDescription',
    'installImmersiveTitle',
    'installImmersiveDescription',
    'installNotifyTitle',
    'installNotifyDescription',
    'iosInstallDescription',
    'installAction',
    'installLater',
    'installClose',
    'installing',
  ] as const
  for (const key of keys) {
    assert.ok(zhCN.pwa[key], `zh-CN missing pwa.${key}`)
    assert.ok(en.pwa[key], `en missing pwa.${key}`)
    assert.match(installTemplate, new RegExp(`pwa\\.${key}`))
  }
  assert.deepEqual({
    title: zhCN.pwa.installTitle,
    subtitle: zhCN.pwa.installSubtitle,
    description: zhCN.pwa.installDescription,
    fast: [zhCN.pwa.installFastTitle, zhCN.pwa.installFastDescription],
    immersive: [zhCN.pwa.installImmersiveTitle, zhCN.pwa.installImmersiveDescription],
    notify: [zhCN.pwa.installNotifyTitle, zhCN.pwa.installNotifyDescription],
    hint: zhCN.pwa.iosInstallDescription,
    primary: zhCN.pwa.installAction,
    later: zhCN.pwa.installLater,
  }, {
    title: '安装 Hippo App',
    subtitle: '添加到主屏幕',
    description: '无需应用商店，获得更快的启动速度、全屏体验和及时通知。',
    fast: ['快速启动', '从主屏幕一键进入'],
    immersive: ['沉浸体验', '全屏浏览，操作更专注'],
    notify: ['及时通知', '重要行情与订单状态不错过'],
    hint: 'iPhone 用户可通过分享菜单添加到主屏幕',
    primary: '立即安装',
    later: '稍后提醒',
  })
  assert.doesNotMatch(installTemplate, /[\u3400-\u9fff]/u)
})
