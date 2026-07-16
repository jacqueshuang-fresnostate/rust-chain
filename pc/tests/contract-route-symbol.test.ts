import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('contract route only resolves symbols from enabled margin products', () => {
  const viewSource = readProjectFile('src/views/Contract.vue')
  const storeSource = readProjectFile('src/stores/contract.ts')

  assert.match(viewSource, /const activeSymbol = computed\(\(\) => contractStore\.activeCoin\?\.symbol \|\| ''\)/)
  assert.match(viewSource, /contractProductsReady/)
  assert.match(viewSource, /routeParamToSymbol/)
  assert.match(viewSource, /trimmed\.replace\(\/\[-_\]\/g,\s*'\/'\)\.toUpperCase\(\)/)
  assert.match(viewSource, /const requestedCoin = routeSymbol \? contractStore\.getCoinBySymbol\(routeSymbol\) : null/)
  assert.match(viewSource, /const resolvedCoin = requestedCoin \|\| contractStore\.coins\[0\] \|\| null/)
  assert.match(viewSource, /router\.replace\(\{\s*name:\s*'Contract',\s*params:\s*\{\s*symbol:\s*urlSymbol\s*\}\s*\}\)/s)
  assert.match(viewSource, /watch\(\(\) => route\.params\.symbol,[\s\S]*if \(!contractProductsReady\.value\) return[\s\S]*resolveContractRouteSymbol\(routeParamToSymbol\(newSymbol\)\)/)
  assert.doesNotMatch(viewSource, /watch\(\(\) => route\.params\.symbol,[\s\S]*\{ immediate: true \}/)

  assert.match(storeSource, /function normalizeContractSymbol\(symbol: string\)/)
  assert.match(storeSource, /symbol\.replace\(\/\[-_\/\]\/g,\s*''\)\.toUpperCase\(\)/)
  assert.match(storeSource, /normalizeContractSymbol\(c\.symbol\) === normalized/)
})
