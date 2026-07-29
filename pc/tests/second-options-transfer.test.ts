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

  // 余额展示已抽成 usdtBalanceText 计算属性，未登录显示 '--'。
  assert.match(source, /usdtBalanceText = computed\(.*formatNumber\(usdtBalance\.value\)\} USDT/)
  assert.match(source, /\{\{ usdtBalanceText \}\}/)
  assert.match(source, /handleOrder\(0\)/)
  assert.match(source, /handleOrder\(1\)/)
})

test('seconds API and store use the shared spot wallet without transfer contracts', () => {
  const apiSource = readProjectFile('src/api/second.ts')
  const storeSource = readProjectFile('src/stores/second.ts')

  assert.match(apiSource, /backendApiUrl\('\/wallet\/accounts'\)/)
  assert.doesNotMatch(apiSource, /SecondTransferParams/)
  assert.doesNotMatch(apiSource, /transferSecondFunds/)
  assert.doesNotMatch(storeSource, /SecondTransferParams/)
  assert.doesNotMatch(storeSource, /transferSecondFunds/)
  assert.doesNotMatch(storeSource, /\basync function transfer\s*\(/)
  assert.doesNotMatch(storeSource, /\btransfer\s*,?\s*\n/)
})
