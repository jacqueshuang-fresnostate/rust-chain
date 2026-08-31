import { access, readFile, readdir, stat } from 'node:fs/promises'
import { relative, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const MAX_STAGE_ASSET_BYTES = 128 * 1024
const TURNSTILE_ORIGIN = 'https://challenges.cloudflare.com'
const BACKEND_API_ORIGIN = 'https://hipoex.cllbmz.kdns.fr'
const BACKEND_WS_ORIGIN = 'wss://hipoex.cllbmz.kdns.fr'

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

async function collectFiles(directory, root = directory) {
  if (!await exists(directory)) return []
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(entries.map(async (entry) => {
    const absolute = resolve(directory, entry.name)
    if (entry.isDirectory()) return collectFiles(absolute, root)
    const metadata = await stat(absolute)
    return [{
      absolute,
      relative: relative(root, absolute).replaceAll('\\', '/'),
      size: metadata.size,
    }]
  }))
  return nested.flat()
}

function normalizeCsp(csp) {
  if (typeof csp === 'string') {
    return Object.fromEntries(csp.split(';').map((part) => part.trim()).filter(Boolean).map((part) => {
      const [directive, ...sources] = part.split(/\s+/)
      return [directive, sources]
    }))
  }
  if (!csp || typeof csp !== 'object' || Array.isArray(csp)) return null
  return Object.fromEntries(Object.entries(csp).map(([directive, sources]) => [
    directive,
    Array.isArray(sources) ? sources.map(String) : String(sources).split(/\s+/).filter(Boolean),
  ]))
}

export function validateTauriCsp(csp) {
  const directives = normalizeCsp(csp)
  if (!directives) return ['Tauri app.security.csp must be a non-null string or directive map']

  const required = {
    'default-src': ["'self'"],
    'base-uri': ["'self'"],
    'object-src': ["'none'"],
    'frame-ancestors': ["'none'"],
    'script-src': ["'self'", TURNSTILE_ORIGIN],
    'style-src': ["'self'", "'unsafe-inline'"],
    'connect-src': [
      "'self'",
      'ipc:',
      'http://ipc.localhost',
      BACKEND_API_ORIGIN,
      BACKEND_WS_ORIGIN,
      TURNSTILE_ORIGIN,
    ],
    'img-src': ["'self'", 'data:', 'blob:', 'https:'],
    'font-src': ["'self'", 'data:', 'blob:'],
    'frame-src': [TURNSTILE_ORIGIN],
    'worker-src': ["'self'", 'blob:'],
  }
  const failures = []
  for (const [directive, sources] of Object.entries(required)) {
    const configured = new Set(directives[directive] || [])
    for (const source of sources) {
      if (!configured.has(source)) failures.push(`Tauri CSP ${directive} is missing ${source}`)
    }
  }
  for (const broadSource of ['https:', 'wss:']) {
    if (directives['connect-src']?.includes(broadSource)) {
      failures.push(`Tauri CSP connect-src must use configured origins instead of broad ${broadSource}`)
    }
  }
  return failures
}

function checkStageAsset(files) {
  const failures = []
  const modern = files.filter((file) => /(?:^|\/)signal-theatre-[^/]+\.webp$/.test(file.relative))
  const legacy = files.filter((file) => /(?:^|\/)signal-theatre-[^/]+\.png$/.test(file.relative))
  if (modern.length !== 1) failures.push(`expected one emitted signal-theatre WebP, found ${modern.length}`)
  if (legacy.length > 0) failures.push(`legacy signal-theatre PNG was emitted: ${legacy.map((file) => file.relative).join(', ')}`)
  for (const file of modern) {
    if (file.size > MAX_STAGE_ASSET_BYTES) {
      failures.push(`${file.relative} is ${(file.size / 1024).toFixed(1)} KiB; limit is ${MAX_STAGE_ASSET_BYTES / 1024} KiB`)
    }
  }
  return { failures, modern }
}

function manifestFailures(manifest) {
  const failures = []
  const expected = {
    name: 'Hippo Mobile',
    short_name: 'Hippo',
    display: 'standalone',
    orientation: 'portrait-primary',
  }
  for (const [key, value] of Object.entries(expected)) {
    if (manifest?.[key] !== value) failures.push(`manifest ${key} must be ${value}`)
  }
  const sizes = new Set((manifest?.icons || []).map((icon) => `${icon.sizes}:${icon.purpose || 'any'}`))
  for (const icon of ['192x192:any', '512x512:any', '512x512:maskable']) {
    if (!sizes.has(icon)) failures.push(`manifest icon ${icon} is missing`)
  }
  return failures
}

async function inspectPwa(dist, files) {
  const failures = []
  const required = [
    'index.html',
    'manifest.webmanifest',
    'sw.js',
    'pwa/icon-192.png',
    'pwa/icon-512.png',
    'pwa/icon-maskable-512.png',
    'pwa/apple-touch-icon.png',
  ]
  const names = new Set(files.map((file) => file.relative))
  for (const file of required) {
    if (!names.has(file)) failures.push(`PWA artifact ${file} is missing`)
  }
  if (!files.some((file) => /^workbox-[^/]+\.js$/.test(file.relative))) {
    failures.push('PWA Workbox runtime artifact is missing')
  }

  if (names.has('index.html')) {
    const html = await readFile(resolve(dist, 'index.html'), 'utf8')
    if (!/<link\b[^>]*rel=["']manifest["']/i.test(html)) failures.push('PWA index.html is missing its manifest link')
  }
  if (names.has('manifest.webmanifest')) {
    try {
      failures.push(...manifestFailures(JSON.parse(await readFile(resolve(dist, 'manifest.webmanifest'), 'utf8'))))
    } catch (error) {
      failures.push(`manifest.webmanifest is invalid JSON: ${error instanceof Error ? error.message : String(error)}`)
    }
  }

  let precacheUrls = []
  if (names.has('sw.js')) {
    const sw = await readFile(resolve(dist, 'sw.js'), 'utf8')
    precacheUrls = [...sw.matchAll(/\burl\s*:\s*["']([^"']+)["']/g)].map((match) => match[1])
    if (sw.includes('signal-theatre')) failures.push('signal-theatre must not be in the unconditional PWA precache')
    for (const url of precacheUrls) {
      if (/\/(?:api|ws|health|downloads?)(?:\/|$)/.test(new URL(url, 'https://mobile.invalid').pathname)) {
        failures.push(`financial/runtime URL entered PWA precache: ${url}`)
      }
    }
  }

  const stage = checkStageAsset(files)
  failures.push(...stage.failures)
  return {
    failures,
    diagnostics: {
      fileCount: files.length,
      precacheCount: precacheUrls.length,
      stageAssets: stage.modern,
    },
  }
}

async function inspectTauri(dist, files, csp) {
  const failures = []
  const names = files.map((file) => file.relative)
  if (!names.includes('index.html')) failures.push('Tauri index.html is missing')
  for (const file of names) {
    if (file === 'manifest.webmanifest' || file === 'sw.js' || /^workbox-[^/]+\.js$/.test(file) || file.startsWith('pwa/')) {
      failures.push(`Tauri build contains forbidden PWA artifact: ${file}`)
    }
  }
  if (names.includes('index.html')) {
    const html = await readFile(resolve(dist, 'index.html'), 'utf8')
    if (/data-pwa-only|rel=["']manifest["']/i.test(html)) failures.push('Tauri index.html retained PWA-only metadata')
  }
  failures.push(...validateTauriCsp(csp))
  const stage = checkStageAsset(files)
  failures.push(...stage.failures)
  return {
    failures,
    diagnostics: {
      fileCount: files.length,
      precacheCount: 0,
      stageAssets: stage.modern,
    },
  }
}

export async function inspectBuildArtifacts(mode, distDirectory, options = {}) {
  if (mode !== 'pwa' && mode !== 'tauri') throw new Error(`unknown build mode: ${mode}`)
  const dist = resolve(distDirectory)
  const files = await collectFiles(dist)
  return mode === 'pwa'
    ? inspectPwa(dist, files)
    : inspectTauri(dist, files, options.csp)
}

export function formatArtifactDiagnostics(mode, diagnostics) {
  const stage = diagnostics.stageAssets.length === 0
    ? 'none'
    : diagnostics.stageAssets.map((file) => `${file.relative} ${(file.size / 1024).toFixed(1)} KiB`).join(', ')
  return [
    `${mode.toUpperCase()} artifact diagnostics:`,
    `  files: ${diagnostics.fileCount}`,
    `  precache entries: ${diagnostics.precacheCount}`,
    `  stage asset: ${stage}`,
  ].join('\n')
}

async function main() {
  const mode = process.argv[2]
  const dist = resolve(process.cwd(), process.argv[3] || 'dist')
  let csp
  if (mode === 'tauri') {
    const config = JSON.parse(await readFile(resolve(process.cwd(), 'src-tauri/tauri.conf.json'), 'utf8'))
    csp = config?.app?.security?.csp
  }
  const result = await inspectBuildArtifacts(mode, dist, { csp })
  console.log(formatArtifactDiagnostics(mode, result.diagnostics))
  if (result.failures.length > 0) {
    console.error('\nArtifact assertion failures:')
    for (const failure of result.failures) console.error(`  - ${failure}`)
    process.exitCode = 1
  } else {
    console.log('\nArtifact assertions passed.')
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`Artifact check failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
