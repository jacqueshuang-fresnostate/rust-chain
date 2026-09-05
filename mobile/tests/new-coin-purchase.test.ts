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
  assert.match(source, /accounts\.value\.find\([\s\S]*account\.assetId === project\.value\?\.quoteAssetId/)
  assert.doesNotMatch(source, /accounts\.value\[0\]|symbol === 'USDT'|<select/)
  assert.match(source, /const budget = decimalPortion\(availableText\.value, percentage, 100, 18\)[\s\S]*decimalDivide\(budget, executionPriceText\.value, 18\)/)
  assert.match(source, /quoteAssetId: project\.value\.quoteAssetId/)
  assert.match(source, /pairId: project\.value\.postListingPairId/)
})
