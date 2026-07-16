import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('pc registers a root-scoped image cache service worker', () => {
  const source = readProjectFile('src/main.ts')

  assert.match(source, /const registerImageCacheWorker = \(\) =>/)
  assert.match(source, /'serviceWorker' in navigator/)
  assert.match(source, /navigator\.serviceWorker\.register\('\/image-cache-sw\.js', \{ scope: '\/' \}\)/)
  assert.match(source, /window\.addEventListener\('load', register, \{ once: true \}\)/)
  assert.match(source, /app\.mount\('#app'\)\s*registerImageCacheWorker\(\)/)
})

test('image cache service worker only caches bounded image GET requests', () => {
  const source = readProjectFile('public/image-cache-sw.js')

  assert.match(source, /const IMAGE_CACHE_NAME = 'pc-image-cache-v1'/)
  assert.match(source, /const MAX_IMAGE_CACHE_ENTRIES = 300/)
  assert.match(source, /request\.method !== 'GET'/)
  assert.match(source, /request\.headers\.has\('range'\)/)
  assert.match(source, /request\.destination === 'image'/)
  assert.match(source, /IMAGE_EXTENSION_PATTERN\.test\(url\.pathname\)/)
  assert.match(source, /caches\.open\(IMAGE_CACHE_NAME\)/)
  assert.match(source, /cache\.match\(event\.request\)/)
  assert.match(source, /cache\.put\(request, response\.clone\(\)\)/)
  assert.match(source, /trimImageCache\(cache\)/)
  assert.match(source, /cacheName\.startsWith\(IMAGE_CACHE_PREFIX\)/)
  assert.doesNotMatch(source, /json|api\/v1|text\/html/i)
})
