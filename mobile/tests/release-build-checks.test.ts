import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import {
  DEFAULT_BUNDLE_BUDGET,
  evaluateBundleBudget,
  formatBundleReport,
  measureBundle,
} from '../scripts/check-bundle-budget.mjs'
import {
  inspectBuildArtifacts,
  validateTauriCsp,
} from '../scripts/check-build-artifacts.mjs'

const VALID_TAURI_CSP = {
  'default-src': ["'self'"],
  'base-uri': ["'self'"],
  'object-src': ["'none'"],
  'frame-ancestors': ["'none'"],
  'script-src': ["'self'", 'https://challenges.cloudflare.com'],
  'style-src': ["'self'", "'unsafe-inline'"],
  'connect-src': [
    "'self'",
    'ipc:',
    'http://ipc.localhost',
    'https://hipoex.cllbmz.kdns.fr',
    'wss://hipoex.cllbmz.kdns.fr',
    'https://challenges.cloudflare.com',
  ],
  'img-src': ["'self'", 'data:', 'blob:', 'https:'],
  'font-src': ["'self'", 'data:', 'blob:'],
  'frame-src': ['https://challenges.cloudflare.com'],
  'worker-src': ["'self'", 'blob:'],
}

async function createBundleFixture(): Promise<string> {
  const dist = await mkdtemp(join(tmpdir(), 'hippo-bundle-'))
  await mkdir(join(dist, 'assets'), { recursive: true })
  await writeFile(join(dist, 'index.html'), [
    '<!doctype html>',
    '<link rel="stylesheet" href="/assets/entry.css">',
    '<script type="module" src="/assets/entry.js"></script>',
  ].join('\n'))
  await writeFile(join(dist, 'assets/entry.js'), 'export const value = 1;\n'.repeat(200))
  await writeFile(join(dist, 'assets/route.js'), 'export const route = 2;\n'.repeat(100))
  await writeFile(join(dist, 'assets/entry.css'), '.fixture{display:block}\n'.repeat(100))
  return dist
}

async function createPwaFixture(): Promise<string> {
  const dist = await mkdtemp(join(tmpdir(), 'hippo-pwa-artifacts-'))
  await mkdir(join(dist, 'assets'), { recursive: true })
  await mkdir(join(dist, 'pwa'), { recursive: true })
  await writeFile(join(dist, 'index.html'), '<link rel="manifest" href="/manifest.webmanifest">')
  await writeFile(join(dist, 'manifest.webmanifest'), JSON.stringify({
    name: 'Hippo Mobile',
    short_name: 'Hippo',
    display: 'standalone',
    orientation: 'portrait-primary',
    icons: [
      { sizes: '192x192', purpose: 'any' },
      { sizes: '512x512', purpose: 'any' },
      { sizes: '512x512', purpose: 'maskable' },
    ],
  }))
  await writeFile(join(dist, 'sw.js'), 'precacheAndRoute([{url:"index.html"},{url:"assets/entry.js"}])')
  await writeFile(join(dist, 'workbox-fixture.js'), 'self.workbox = true')
  await writeFile(join(dist, 'assets/entry.js'), 'export{}')
  await writeFile(join(dist, 'assets/signal-theatre-fixture.webp'), Buffer.alloc(1024))
  for (const icon of ['icon-192.png', 'icon-512.png', 'icon-maskable-512.png', 'apple-touch-icon.png']) {
    await writeFile(join(dist, 'pwa', icon), Buffer.alloc(8))
  }
  return dist
}

test('bundle budget measures emitted raw/gzip bytes and reports actionable failures', async () => {
  const dist = await createBundleFixture()
  try {
    const report = await measureBundle(dist)
    assert.equal(report.entries.js[0]?.relative, 'entry.js')
    assert.equal(report.entries.css[0]?.relative, 'entry.css')
    assert.equal(evaluateBundleBudget(report).length, 0)

    const failingBudget = structuredClone(DEFAULT_BUNDLE_BUDGET)
    failingBudget.totals.js.raw = 1
    failingBudget.totals.js.gzip = 1
    const failures = evaluateBundleBudget(report, failingBudget)
    assert.ok(failures.some((failure) => failure.includes('total JS raw')))
    assert.ok(failures.some((failure) => failure.includes('total JS gzip')))
    const diagnostics = formatBundleReport(report, failingBudget)
    assert.match(diagnostics, /entry\.js: .* raw \/ .* gzip/)
    assert.match(diagnostics, /JS limits:/)
  } finally {
    await rm(dist, { recursive: true, force: true })
  }
})

test('PWA artifact assertions inspect generated files and reject stage precaching', async () => {
  const dist = await createPwaFixture()
  try {
    const passing = await inspectBuildArtifacts('pwa', dist)
    assert.deepEqual(passing.failures, [])
    assert.equal(passing.diagnostics.precacheCount, 2)

    await writeFile(
      join(dist, 'sw.js'),
      'precacheAndRoute([{url:"index.html"},{url:"assets/signal-theatre-fixture.webp"}])',
    )
    const failing = await inspectBuildArtifacts('pwa', dist)
    assert.ok(failing.failures.some((failure) => failure.includes('unconditional PWA precache')))
  } finally {
    await rm(dist, { recursive: true, force: true })
  }
})

test('Tauri artifact assertions reject PWA output and enforce an explicit functional CSP', async () => {
  const dist = await mkdtemp(join(tmpdir(), 'hippo-tauri-artifacts-'))
  try {
    await mkdir(join(dist, 'assets'), { recursive: true })
    await writeFile(join(dist, 'index.html'), '<script type="module" src="/assets/entry.js"></script>')
    await writeFile(join(dist, 'assets/entry.js'), 'export{}')
    await writeFile(join(dist, 'assets/signal-theatre-fixture.webp'), Buffer.alloc(1024))

    assert.deepEqual(validateTauriCsp(VALID_TAURI_CSP), [])
    assert.ok(validateTauriCsp(null).some((failure) => failure.includes('non-null')))
    const broadConnectCsp = structuredClone(VALID_TAURI_CSP)
    broadConnectCsp['connect-src'] = ["'self'", 'ipc:', 'http://ipc.localhost', 'https:', 'wss:', 'https://challenges.cloudflare.com']
    assert.ok(validateTauriCsp(broadConnectCsp).some((failure) => failure.includes('configured origins')))
    const passing = await inspectBuildArtifacts('tauri', dist, { csp: VALID_TAURI_CSP })
    assert.deepEqual(passing.failures, [])

    await writeFile(join(dist, 'sw.js'), 'self.addEventListener("fetch", () => {})')
    const failing = await inspectBuildArtifacts('tauri', dist, { csp: VALID_TAURI_CSP })
    assert.ok(failing.failures.some((failure) => failure.includes('forbidden PWA artifact: sw.js')))
  } finally {
    await rm(dist, { recursive: true, force: true })
  }
})
