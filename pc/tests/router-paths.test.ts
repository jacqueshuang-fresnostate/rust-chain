import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('uses /spot as the PC spot trading route and entry path', () => {
  const routerSource = readProjectFile('src/router/index.ts')
  const homeSource = readProjectFile('src/views/Home.vue')
  const agentGuide = readProjectFile('AGENT.md')

  assert.match(routerSource, /path:\s*['"]spot\/:symbol\?['"][\s\S]*name:\s*['"]Trade['"]/)
  assert.doesNotMatch(routerSource, /path:\s*['"]trade\/:symbol\?['"]/)
  assert.match(homeSource, /\$router\.push\(['"]\/spot['"]\)/)
  assert.doesNotMatch(homeSource, /\$router\.push\(['"]\/trade['"]\)/)
  assert.match(agentGuide, /\/spot\/BTC_USDT/)
  assert.doesNotMatch(agentGuide, /\/trade\/BTC_USDT/)
})

test('redirects the unfinished OTC route instead of loading a placeholder page', () => {
  const routerSource = readProjectFile('src/router/index.ts')

  assert.match(routerSource, /path:\s*['"]otc['"][\s\S]*redirect:\s*['"]\/market['"]/)
  assert.doesNotMatch(routerSource, /views\/OTC\.vue/)
  assert.throws(() => readProjectFile('src/views/OTC.vue'), /ENOENT/)
})

test('removes the old binary options mock page from the active PC codebase', () => {
  const routerSource = readProjectFile('src/router/index.ts')

  assert.doesNotMatch(routerSource, /BinaryOptions/)
  assert.throws(() => readProjectFile('src/views/BinaryOptions.vue'), /ENOENT/)
})

test('keeps business websocket connections scoped to their own pages', () => {
  const layoutSource = readProjectFile('src/components/layout/MainLayout.vue')
  const spotSource = readProjectFile('src/views/Trade.vue')
  const marginSource = readProjectFile('src/views/Contract.vue')
  const secondsSource = readProjectFile('src/views/SecondOptions.vue')

  assert.doesNotMatch(layoutSource, /stompService\.connect\(['"]spot['"]\)/)
  assert.match(spotSource, /stompService\.connect\(['"]spot['"]\)/)
  assert.match(marginSource, /stompService\.connect\(['"]margin['"]\)/)
  assert.match(secondsSource, /stompService\.connect\(['"]seconds['"]\)/)
})
