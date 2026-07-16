import assert from 'node:assert/strict'
import test from 'node:test'
import { detectClientPlatform } from '../src/core/platform.ts'

test('H5 平台识别覆盖 Android、iOS 与桌面浏览器', () => {
  assert.equal(detectClientPlatform('Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 Mobile'), 'mobile_web')
  assert.equal(detectClientPlatform('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)'), 'mobile_web')
  assert.equal(detectClientPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)'), 'desktop_web')
})
