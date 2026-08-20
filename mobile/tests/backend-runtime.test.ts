import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import {
  BackendConfigurationError,
  DEFAULT_BACKEND_DEV_PROXY_TARGET,
  resolveBackendApiUrl,
  resolveBackendDevProxyTarget,
  resolveBackendHealthUrl,
  resolveBackendRuntimeConfig,
  resolveBackendWebSocketUrl,
  resolvePrivateUserWebSocketUrl,
} from '../src/config/backend.ts'
import {
  PRODUCT_BACKEND_ORIGIN,
  resolveProductBackendOrigin,
} from '../src/config/product.ts'

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
  assert.equal(
    resolvePrivateUserWebSocketUrl(
      runtime,
      ' access token/?= ',
      'http://127.0.0.1:1611',
    ),
    'ws://127.0.0.1:1611/api/v1/ws/private?token=access%20token%2F%3F%3D',
  )
  assert.equal(
    resolvePrivateUserWebSocketUrl(runtime, '   ', 'http://127.0.0.1:1611'),
    null,
  )
  assert.equal(DEFAULT_BACKEND_DEV_PROXY_TARGET, PRODUCT_BACKEND_ORIGIN)
  assert.equal(resolveBackendDevProxyTarget(), PRODUCT_BACKEND_ORIGIN)
  assert.equal(resolveBackendDevProxyTarget('http://127.0.0.1:18080'), 'http://127.0.0.1:18080')
  assert.equal(resolveBackendApiUrl(runtime, '/markets'), '/api/v1/markets')
})

test('mobile product builds inject the remote backend while non-empty environment values retain priority', async () => {
  assert.equal(resolveProductBackendOrigin(undefined), 'https://hipoex.cllbmz.kdns.fr')
  assert.equal(resolveProductBackendOrigin('   '), 'https://hipoex.cllbmz.kdns.fr')
  assert.equal(
    resolveProductBackendOrigin(' https://api.example.test '),
    'https://api.example.test',
  )

  for (const native of [false, true]) {
    const runtime = resolveBackendRuntimeConfig({
      apiDomain: resolveProductBackendOrigin(undefined),
      dev: false,
      native,
    })
    assert.equal(
      resolveBackendApiUrl(runtime, '/markets'),
      'https://hipoex.cllbmz.kdns.fr/api/v1/markets',
    )
    assert.equal(
      resolveBackendWebSocketUrl(runtime, '/ws/public'),
      'wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public',
    )
    assert.equal(
      resolvePrivateUserWebSocketUrl(runtime, 'TOKEN'),
      'wss://hipoex.cllbmz.kdns.fr/api/v1/ws/private?token=TOKEN',
    )
  }

  const overridden = resolveBackendRuntimeConfig({
    apiDomain: resolveProductBackendOrigin(' https://api.example.test '),
    dev: false,
    native: true,
  })
  assert.equal(resolveBackendApiUrl(overridden, '/markets'), 'https://api.example.test/api/v1/markets')
  assert.equal(resolveBackendWebSocketUrl(overridden, '/ws/public'), 'wss://api.example.test/api/v1/ws/public')
  assert.equal(
    resolvePrivateUserWebSocketUrl(overridden, 'TOKEN'),
    'wss://api.example.test/api/v1/ws/private?token=TOKEN',
  )

  const appSource = await readFile(new URL('../src/config/app.ts', import.meta.url), 'utf8')
  assert.match(appSource, /apiDomain:\s*resolveProductBackendOrigin\(env\.VITE_BACKEND_API_DOMAIN\)/)
  assert.match(appSource, /resolveBackendWebSocketUrl\(APP_CONFIG\.backend,\s*'\/ws\/public'/)
  assert.match(appSource, /export function privateUserWebSocketUrl\(/)
  assert.match(appSource, /resolvePrivateUserWebSocketUrl\(APP_CONFIG\.backend, accessToken, pageOrigin\)/)
})

test('mobile startup does not use the challenged health endpoint as an availability gate', async () => {
  const startupSources = await Promise.all([
    readFile(new URL('../src/main.ts', import.meta.url), 'utf8'),
    readFile(new URL('../src/App.vue', import.meta.url), 'utf8'),
  ])
  for (const source of startupSources) {
    assert.doesNotMatch(source, /backendHealthUrl|['"`]\/health(?:\/|['"`])/)
  }
})

test('generic runtime resolver retains same-origin browser and missing-native contracts', () => {
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
  assert.match(envSource, /VITE_BACKEND_DEV_PROXY_TARGET=https:\/\/hipoex\.cllbmz\.kdns\.fr/)
  assert.match(envSource, /VITE_BACKEND_API_DOMAIN=https:\/\/hipoex\.cllbmz\.kdns\.fr/)
  assert.match(readme, /VITE_BACKEND_DEV_PROXY_TARGET/)
  assert.match(readme, /18080/)
  assert.match(readme, /客户端启动和业务页面展示不以 `\/health` 为门禁/)
  assert.match(readme, /wss:\/\/hipoex\.cllbmz\.kdns\.fr\/api\/v1\/ws\/public/)
})
