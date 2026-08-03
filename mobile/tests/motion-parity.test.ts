import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const appSource = read('../src/App.vue')
const signalFieldSource = read('../src/components/SignalField.vue')
const navigationSource = read('../src/core/navigation.ts')
const routerSource = read('../src/router/index.ts')
const baseStyles = read('../src/styles/prototype-base.css')
const parityStyles = read('../src/styles/prototype-parity.css')

test('SignalField 等价保留原型波形、粒子、扫描带与双主题颜色', () => {
  assert.match(signalFieldSource, /const MAX_SIGNAL_DPR = 2/)
  assert.match(signalFieldSource, /const MAX_SIGNAL_PIXELS = 2_200_000/)
  assert.match(signalFieldSource, /Array\.from\(\{ length: 28 \}/)
  assert.match(signalFieldSource, /for \(let line = 0; line < 4; line \+= 1\)/)
  assert.match(signalFieldSource, /x \+= 34/)
  assert.match(signalFieldSource, /y \+= 34/)
  assert.match(signalFieldSource, /\* 0\.065/)
  assert.match(signalFieldSource, /rgba\(0, 126, 84, \.46\)/)
  assert.match(signalFieldSource, /rgba\(0, 104, 204, \.34\)/)
  assert.match(signalFieldSource, /rgba\(218, 62, 52, \.24\)/)
  assert.match(signalFieldSource, /rgba\(84, 255, 181, \.72\)/)
  assert.match(signalFieldSource, /rgba\(55, 157, 255, \.5\)/)
  assert.match(signalFieldSource, /rgba\(255, 91, 75, \.34\)/)
  assert.match(signalFieldSource, /createLinearGradient\(0, scanY - 30, 0, scanY \+ 30\)/)
  assert.match(signalFieldSource, /pointer\.active \? 18 : 10/)
})

test('SignalField 限制画布像素并完整处理低动态、隐藏、尺寸与卸载', () => {
  assert.match(signalFieldSource, /Math\.sqrt\(MAX_SIGNAL_PIXELS \//)
  assert.match(signalFieldSource, /Math\.min\(window\.devicePixelRatio \|\| 1, MAX_SIGNAL_DPR, pixelCapRatio\)/)
  assert.match(signalFieldSource, /setTransform\(ratio, 0, 0, ratio, 0, 0\)/)
  assert.match(signalFieldSource, /prefers-reduced-motion: reduce/)
  assert.match(signalFieldSource, /const time = reduced \? REDUCED_SIGNAL_TIMESTAMP : timestamp/)
  assert.match(signalFieldSource, /if \(document\.hidden \|\| reduced\) return/)
  assert.match(signalFieldSource, /addEventListener\('visibilitychange', onVisibilityChange\)/)
  assert.match(signalFieldSource, /addEventListener\('resize', onResize\)/)
  assert.match(signalFieldSource, /addEventListener\('pointermove', onPointerMove/)
  assert.match(signalFieldSource, /addEventListener\('pointerdown', onPointerMove/)
  assert.match(signalFieldSource, /cancelAnimationFrame\(resizeId\)/)
  assert.match(signalFieldSource, /removeEventListener\('visibilitychange', onVisibilityChange\)/)
  assert.match(signalFieldSource, /motionQuery\.removeEventListener\('change', onMotionChange\)/)
  assert.match(signalFieldSource, /<span class="signal-static-fallback" aria-hidden="true" \/>/)
  assert.match(signalFieldSource, /<canvas ref="canvasRef" class="signal-field" aria-hidden="true" \/>/)
})

test('应用壳只在表现型根页挂载背景并持续渲染方向幕帘', () => {
  assert.match(appSource, /import SignalField from '@\/components\/SignalField\.vue'/)
  assert.match(
    appSource,
    /\['home', 'markets'\]\.includes\(String\(route\.name \|\| ''\)\)/,
  )
  assert.match(appSource, /<div v-if="showSignalField" class="ambient-layer" aria-hidden="true">/)
  assert.match(appSource, /<SignalField :light="!theme\.isDark" \/>/)
  assert.match(appSource, /:key="`veil-\$\{routeTransitionSequence\}`"/)
  assert.match(appSource, /:class="`route-veil-\$\{routeTransitionTier\}`"/)
  assert.match(appSource, /:data-direction="routeDirection"/)
  assert.match(appSource, /'view-stack',[\s\S]*?\.\.\.routeMotionClasses/)
  assert.match(appSource, /:data-motion-zone="showSignalField \? 'expressive' : 'protected'"/)
})

test('路由守卫把根栏目身份与完整路径变化写入共享动效状态', () => {
  assert.match(navigationSource, /export const routeDirection = ref<RouteDirection>\('still'\)/)
  assert.match(navigationSource, /export const routeTransitionTier = ref<RouteTransitionTier>\('secondary'\)/)
  assert.match(navigationSource, /export const routeTransitionSequence = ref\(0\)/)
  assert.match(routerSource, /resolveRootRouteKey\(to\.name, to\.query\.mode, to\.query\.purpose\)/)
  assert.match(routerSource, /resolveRootRouteKey\(from\.name, from\.query\.mode, from\.query\.purpose\)/)
  assert.match(routerSource, /to\.fullPath !== from\.fullPath/)
})

test('生产壳消费原型精确幕帘、位移动画、层级与低动态规则', () => {
  assert.match(baseStyles, /\.route-veil-root\s*\{[\s\S]*?360ms linear both/)
  assert.match(baseStyles, /\.route-veil-root\[data-direction="back"\] span/)
  assert.match(baseStyles, /\.view-stack\.transition-root\.route-forward\s*\{[\s\S]*?280ms/)
  assert.match(baseStyles, /\.view-stack\.transition-root\.route-back\s*\{[\s\S]*?280ms/)
  assert.match(baseStyles, /\.view-stack\.transition-secondary\.route-forward\s*\{[\s\S]*?180ms/)
  assert.match(baseStyles, /\.view-stack\.transition-secondary\.route-back\s*\{[\s\S]*?170ms/)
  assert.match(baseStyles, /\.topbar,[\s\S]*?\.secondary-header\s*\{[\s\S]*?z-index:\s*70/)
  assert.match(baseStyles, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.route-veil\s*\{[\s\S]*?display:\s*none !important/)
  assert.match(parityStyles, /\.route-forward-enter-active,[\s\S]*?z-index:\s*var\(--layer-route-transition\)/)
  assert.match(parityStyles, /\.route-forward-leave-active,[\s\S]*?z-index:\s*var\(--layer-content\)/)
})
