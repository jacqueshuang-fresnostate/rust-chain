import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const source = readFileSync(new URL('../src/views/LoginView.vue', import.meta.url), 'utf8')
const lifecycleSource = readFileSync(new URL('../src/core/turnstile.ts', import.meta.url), 'utf8')

function functionSource(name: string, nextName: string): string {
  const start = source.indexOf(`function ${name}`)
  const end = source.indexOf(`function ${nextName}`, start)
  assert.notEqual(start, -1, `${name} must exist`)
  assert.notEqual(end, -1, `${nextName} must exist after ${name}`)
  return source.slice(start, end)
}

test('Turnstile uses explicit responsive rendering with the current app theme and language', () => {
  assert.match(lifecycleSource, /api\.js\?render=explicit/)
  assert.match(lifecycleSource, /api\.ready\(\(\) => resolve\(api\)\)/)
  assert.match(source, /size: 'flexible'/)
  assert.match(source, /theme: widgetTheme/)
  assert.match(source, /appearance: 'always'/)
  assert.match(source, /language: widgetLanguage/)
  assert.match(source, /beforeInteractive: \(\) => \{[\s\S]*turnstileStatus\.value = 'ready'/)
  assert.match(lifecycleSource, /'before-interactive-callback': runIfCurrent/)
  assert.match(source, /theme\.theme === 'dark' \? 'dark' : 'light'/)
  assert.match(source, /locale\.value === 'en' \? 'en' : 'zh-cn'/)
  assert.match(source, /watch\(\[turnstileTheme, turnstileLanguage\]/)
})

test('Turnstile lifecycle accepts widget id zero and keeps an existing widget after reset', () => {
  const resetSource = functionSource('resetCfTurnstileWidget', 'removeTurnstileWidget')
  const removeSource = functionSource('removeTurnstileWidget', 'initializeTurnstile')

  assert.match(resetSource, /if \(!turnstileLifecycle\.reset\(\)\)/)
  assert.match(resetSource, /turnstileStatus\.value = 'ready'[\s\S]*return true/)
  assert.match(removeSource, /turnstileLifecycle\.remove\(\)/)
  assert.match(lifecycleSource, /currentWidgetId !== null && currentApi/)
  assert.match(lifecycleSource, /currentApi\.reset\(currentWidgetId\)[\s\S]*return true/)
  assert.doesNotMatch(resetSource, /!turnstileWidgetId\.value/)
})

test('Turnstile loader and lifecycle are module scoped and generation guarded', () => {
  assert.match(source, /createTurnstileLifecycle\(/)
  assert.doesNotMatch(source, /turnstileScriptPromise/)
  assert.match(lifecycleSource, /let turnstileLoadPromise: Promise<TurnstileApi> \| null = null/)
  assert.match(lifecycleSource, /document\.querySelector<HTMLScriptElement>\(TURNSTILE_SCRIPT_SELECTOR\)/)
  assert.match(lifecycleSource, /turnstileLoadPromise = null/)
  assert.match(lifecycleSource, /generation \+= 1[\s\S]*removeRenderedWidget\(\)/)
  assert.match(lifecycleSource, /container\.isConnected && isContainerCurrent\(container\)/)
  assert.match(lifecycleSource, /if \(!isCurrent\(renderGeneration, currentContainer/)
})

test('Turnstile presents truthful accessible loading, ready, verified, expired, and error states', () => {
  for (const state of ['loading', 'ready', 'expired', 'error']) {
    assert.match(source, new RegExp(`turnstileStatus\\.value = '${state}'`))
  }
  assert.match(source, /:data-state="turnstileStatus"/)
  assert.match(source, /id="auth-turnstile-status" class="sr-only" aria-live="polite"/)
  assert.match(source, /<LoaderCircle/)
  assert.match(source, /turnstileWidgetId === null/)
  assert.match(source, /token \? 'verified' : 'ready'/)
})

test('Turnstile mobile shell stays flexible without scaling or clipping the challenge iframe', () => {
  assert.doesNotMatch(source, /auth-cf-turnstile-wrap/)
  assert.match(source, /\.auth-cf-turnstile \{[\s\S]*justify-content: center;[\s\S]*min-height: 70px;[\s\S]*min-width: 0;[\s\S]*overflow: visible;/)
  assert.match(source, /\.auth-cf-turnstile-loading \{[\s\S]*pointer-events: none;/)
  assert.match(source, /\.cf-turnstile-widget :deep\(iframe\) \{[\s\S]*max-width: 100% !important;/)
  assert.doesNotMatch(source, /\.cf-turnstile-widget \{[\s\S]*?transform:/)
  assert.match(source, /@media \(max-width: 340px\)[\s\S]*\.auth-cf-turnstile \{[\s\S]*margin-inline: -7px;[\s\S]*width: calc\(100% \+ 14px\);/)
  assert.match(source, /class="auth-cf-turnstile-spinner"/)
})

test('Turnstile status copy exists in both mobile locales', () => {
  const keys = [
    'turnstileTitle',
    'turnstileLoading',
    'turnstileReady',
    'turnstileVerified',
    'turnstileExpired',
    'turnstileError',
  ] as const

  for (const key of keys) {
    assert.equal(typeof zhCN.auth[key], 'string', `zh-CN missing auth.${key}`)
    assert.equal(typeof en.auth[key], 'string', `en missing auth.${key}`)
  }
})
