import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import {
  BackendConfigurationError,
  resolveBackendApiUrl,
  resolveBackendDevProxyTarget,
  resolveBackendHealthUrl,
  resolveBackendRuntimeConfig,
  resolveBackendWebSocketUrl,
} from '../src/config/backend.ts'

test('development stays browser same-origin while the Vite proxy target is independently configurable', () => {
  const runtime = resolveBackendRuntimeConfig({
    apiDomain: 'https://legacy-api.example.test',
    dev: true,
  })
  assert.equal(resolveBackendApiUrl(runtime, '/markets'), '/api/v1/markets')
  assert.equal(resolveBackendHealthUrl(runtime), '/health')
  assert.equal(
    resolveBackendWebSocketUrl(runtime, '/ws/public', 'http://127.0.0.1:1611'),
    'ws://127.0.0.1:1611/api/v1/ws/public',
  )
  assert.equal(resolveBackendDevProxyTarget(), 'http://127.0.0.1:8080')
  assert.equal(resolveBackendDevProxyTarget('http://127.0.0.1:18080'), 'http://127.0.0.1:18080')
  assert.equal(resolveBackendApiUrl(runtime, '/markets'), '/api/v1/markets')
})

test('PWA production defaults to same-origin API, health, and nested secure WebSocket paths', () => {
  const runtime = resolveBackendRuntimeConfig({ dev: false, native: false })
  assert.equal(resolveBackendApiUrl(runtime, 'news'), '/api/v1/news')
  assert.equal(resolveBackendHealthUrl(runtime), '/health')
  assert.equal(
    resolveBackendWebSocketUrl(runtime, '/ws/public', 'https://mobile.example.test'),
    'wss://mobile.example.test/api/v1/ws/public',
  )
})

test('explicit production origins must be HTTPS and cannot target device loopback', () => {
  const configured = resolveBackendRuntimeConfig({
    apiDomain: 'https://api.example.test/',
    dev: false,
    native: true,
  })
  assert.equal(resolveBackendApiUrl(configured, '/user/profile'), 'https://api.example.test/api/v1/user/profile')
  assert.equal(resolveBackendHealthUrl(configured), 'https://api.example.test/health')

  for (const apiDomain of ['http://127.0.0.1:8080', 'http://api.example.test']) {
    const invalid = resolveBackendRuntimeConfig({ apiDomain, dev: false, native: true })
    assert.throws(() => resolveBackendApiUrl(invalid, '/markets'), BackendConfigurationError)
  }
})

test('unconfigured Tauri production fails diagnostically instead of selecting loopback', () => {
  const runtime = resolveBackendRuntimeConfig({ dev: false, native: true })
  assert.throws(
    () => resolveBackendApiUrl(runtime, '/markets'),
    (error: unknown) => error instanceof BackendConfigurationError
      && /VITE_BACKEND_API_DOMAIN/.test(error.message),
  )
})

test('Vite proxy uses its dedicated target and forwards API WebSocket upgrades', async () => {
  const viteSource = await readFile(new URL('../vite.config.ts', import.meta.url), 'utf8')
  const envSource = await readFile(new URL('../.env.example', import.meta.url), 'utf8')
  const readme = await readFile(new URL('../README.md', import.meta.url), 'utf8')

  assert.match(viteSource, /VITE_BACKEND_DEV_PROXY_TARGET/)
  assert.match(viteSource, /\[apiPrefix\]:\s*\{[\s\S]*?ws: true/)
  assert.match(viteSource, /'\/health':\s*\{/)
  assert.match(envSource, /VITE_BACKEND_DEV_PROXY_TARGET=http:\/\/127\.0\.0\.1:8080/)
  assert.match(readme, /VITE_BACKEND_DEV_PROXY_TARGET/)
  assert.match(readme, /18080/)
})
