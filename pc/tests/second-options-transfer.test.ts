import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('seconds options page does not expose transfer actions', () => {
  const source = readProjectFile('src/views/SecondOptions.vue')

  for (const pattern of [
    /showTransferModal/,
    /transferDirection/,
    /transferAmount/,
    /transferring/,
    /toggleTransferDirection/,
    /confirmTransfer/,
    /store\.transfer\(/,
    /seconds\.transfer_funds/,
    /Transfer Modal/,
    /SPOT_TO_SECOND/,
    /SECOND_TO_SPOT/,
    /lucide:arrow-right-left/,
  ]) {
    assert.doesNotMatch(source, pattern)
  }

  assert.match(source, /formatNumber\(usdtBalance\)\s*\}\}\s*USDT/)
  assert.match(source, /handleOrder\(0\)/)
  assert.match(source, /handleOrder\(1\)/)
})
