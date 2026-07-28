import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { newCoinPurchaseQuantity } from '../src/core/newCoinPurchase.ts'

test('purchase percentages convert quote balance into base quantity at execution price', () => {
  assert.equal(newCoinPurchaseQuantity(1_000, 0.25, 20), 12.5)
  assert.equal(newCoinPurchaseQuantity(1_000, 1, 20), 50)
  assert.equal(newCoinPurchaseQuantity(1_000, 2, 20), 50)
  assert.equal(newCoinPurchaseQuantity(1_000, 1, 0), 0)
})

test('new-coin purchase locks payment to the authoritative pair quote asset', async () => {
  const source = await readFile(new URL('../src/views/NewCoinDetailView.vue', import.meta.url), 'utf8')
  assert.match(source, /canPurchase\.value[\s\S]*accounts\.value\.find\(\(account\) => account\.symbol === selectedTicker\.value\?\.quote\)/)
  assert.match(source, /<select v-if="canSubscribe" v-model="quoteAssetId">/)
  assert.match(source, /newCoinPurchaseQuantity\(available, value, executionPrice\.value\)/)
  assert.match(source, /pairId: project\.value\.postListingPairId/)
})
