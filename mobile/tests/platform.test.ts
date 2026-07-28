import assert from 'node:assert/strict'
import test from 'node:test'
import { detectClientPlatform, isTauriRuntime } from '../src/core/platform.ts'

test('H5 平台识别覆盖 Android、iOS 与桌面浏览器', () => {
  assert.equal(detectClientPlatform('Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 Mobile'), 'mobile_web')
  assert.equal(detectClientPlatform('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)'), 'mobile_web')
  assert.equal(detectClientPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)'), 'desktop_web')
})

test('Tauri 运行时检测不依赖移动端 user agent', () => {
  const tauriGlobal = { __TAURI_INTERNALS__: {} }
  assert.equal(isTauriRuntime(tauriGlobal), true)
  assert.equal(isTauriRuntime({}), false)
  assert.equal(
    detectClientPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)', tauriGlobal),
    'desktop_web',
  )
  assert.equal(
    detectClientPlatform('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)', tauriGlobal),
    'ios_app',
  )
})
