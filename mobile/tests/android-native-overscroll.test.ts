import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const mainActivity = read('../src-tauri/android/MainActivity.kt')
const androidRunner = read('../scripts/run-android-tauri.mjs')

test('Android WebView disables native edge stretch without intercepting touch input', () => {
  assert.match(mainActivity, /override fun onWebViewCreate\(webView: WebView\)/)
  assert.match(mainActivity, /super\.onWebViewCreate\(webView\)/)
  assert.match(mainActivity, /webView\.overScrollMode = View\.OVER_SCROLL_NEVER/)
  assert.doesNotMatch(mainActivity, /setOnTouchListener|onTouchEvent|requestDisallowInterceptTouchEvent/)
})

test('Android runner synchronizes the tracked Activity around generated project commands', () => {
  assert.match(androidRunner, /src-tauri', 'android', 'MainActivity\.kt'/)
  assert.match(androidRunner, /src-tauri',\s*'gen',[\s\S]*?'MainActivity\.kt'/)
  assert.match(androidRunner, /if \(command !== 'init'\) \{\s*syncMainActivity\(\)/)
  assert.match(
    androidRunner,
    /if \(command === 'init' && child\.status === 0\) \{\s*syncMainActivity\(\)/,
  )
})
