import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  LAUNCH_INTRO_SESSION_KEY,
  rememberLaunchIntro,
  shouldPlayLaunchIntro,
  type LaunchIntroStorage,
} from '../src/core/launchIntro.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const appSource = read('../src/App.vue')
const componentSource = read('../src/components/LaunchIntro.vue')
const baseStyles = read('../src/styles/base.css')
const packageJson = JSON.parse(read('../package.json')) as {
  dependencies?: Record<string, string>
}

test('启动首屏使用版本化会话键且同一会话只播放一次', () => {
  const values = new Map<string, string>()
  const storage: LaunchIntroStorage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }

  assert.equal(LAUNCH_INTRO_SESSION_KEY, 'hippo_mobile_launch_intro_v1')
  assert.equal(shouldPlayLaunchIntro(storage), true)
  rememberLaunchIntro(storage)
  assert.equal(shouldPlayLaunchIntro(storage), false)
})

test('会话存储受限时启动判断和记录均不会阻断应用', () => {
  const restrictedStorage: LaunchIntroStorage = {
    getItem: () => {
      throw new Error('blocked')
    },
    setItem: () => {
      throw new Error('blocked')
    },
  }

  assert.equal(shouldPlayLaunchIntro(null), true)
  assert.equal(shouldPlayLaunchIntro(restrictedStorage), true)
  assert.doesNotThrow(() => rememberLaunchIntro(restrictedStorage))
})

test('生产应用只挂载一个独立 GSAP 启动组件', () => {
  assert.match(packageJson.dependencies?.gsap ?? '', /^\^?\d+\.\d+\.\d+$/)
  assert.match(componentSource, /import \{ gsap \} from 'gsap'/)
  assert.match(appSource, /import LaunchIntro from '@\/components\/LaunchIntro\.vue'/)
  assert.equal((appSource.match(/<LaunchIntro \/>/g) ?? []).length, 1)
  assert.match(componentSource, /gsap\.context\(/)
  assert.match(componentSource, /gsap\.timeline\(/)
})

test('动画使用克制的品牌揭示、扫光、细进度线和左右幕帘离场', () => {
  assert.match(componentSource, /hippo-logo-compact\.png/)
  assert.match(componentSource, /launch-intro__logo-window/)
  assert.match(componentSource, /clipPath: 'inset\(0 0% 0 0%\)'/)
  assert.match(componentSource, /launch-intro__light-pass/)
  assert.match(componentSource, /launch-intro__progress-fill/)
  assert.match(componentSource, /launch-intro__signature/)
  assert.match(componentSource, /launch-intro__curtain--left/)
  assert.match(componentSource, /launch-intro__curtain--right/)
  assert.doesNotMatch(componentSource, /launch-intro__grid|launch-intro__corner|counterRef|SIGNAL ACQUISITION/)
  assert.match(componentSource, /\.to\(root,[\s\S]*?duration: 0\.04,[\s\S]*?\}, 2\.02\)/)
})

test('启动层处理低动态、安全区、最高层级和完整清理', () => {
  assert.match(componentSource, /prefers-reduced-motion: reduce/)
  assert.match(componentSource, /finishIntro\(\)/)
  assert.match(componentSource, /document\.documentElement\.classList\.add\(SCROLL_LOCK_CLASS\)/)
  assert.match(componentSource, /document\.documentElement\.classList\.remove\(SCROLL_LOCK_CLASS\)/)
  assert.match(componentSource, /timeline\?\.kill\(\)/)
  assert.match(componentSource, /animationContext\?\.revert\(\)/)
  assert.match(componentSource, /onBeforeUnmount\(/)
  assert.match(componentSource, /\.launch-intro\s*\{[\s\S]*?inset:\s*0;/)
  assert.match(componentSource, /env\(safe-area-inset-left\)/)
  assert.match(componentSource, /env\(safe-area-inset-right\)/)
  assert.match(componentSource, /aria-hidden="true"/)
  assert.doesNotMatch(componentSource, /<button|<svg|[\u{1F300}-\u{1FAFF}]/u)
  assert.match(baseStyles, /--layer-overlay:\s*80;[\s\S]*?--layer-launch:\s*120;/)
  assert.match(componentSource, /z-index:\s*var\(--layer-launch\)/)
})

test('启动层在 GSAP 未完成或初始化异常时仍会确定性自动移除', () => {
  assert.match(componentSource, /const AUTO_DISMISS_MS = 3000/)
  assert.match(componentSource, /autoDismissTimer = window\.setTimeout\(finishIntro, AUTO_DISMISS_MS\)/)
  assert.match(componentSource, /function finishIntro\(\)[\s\S]*?clearAutoDismissTimer\(\)[\s\S]*?isVisible\.value = false/)
  assert.match(componentSource, /try \{[\s\S]*?gsap\.context\([\s\S]*?\} catch \{\s*finishIntro\(\)\s*\}/)
  assert.match(componentSource, /onBeforeUnmount\(\(\) => \{\s*clearAutoDismissTimer\(\)/)
})
