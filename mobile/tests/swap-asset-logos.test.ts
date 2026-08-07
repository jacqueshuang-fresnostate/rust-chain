import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const swapSource = readFileSync(new URL('../src/views/SwapView.vue', import.meta.url), 'utf8')

test('闪兑使用钱包账户返回的真实资产图片覆盖主卡片与资产选择器', () => {
  assert.match(swapSource, /const accountBySymbol = computed\(\(\) => new Map\(/)
  assert.match(swapSource, /account\.symbol\.trim\(\)\.toUpperCase\(\)/)
  assert.match(swapSource, /const assetLogoUrl = \(symbol: string\): string \| undefined => accountBySymbol\.value\.get\(symbol\.trim\(\)\.toUpperCase\(\)\)\?\.logoUrl/)
  assert.match(swapSource, /<AssetMark :symbol="selectedPair\.fromAssetSymbol" :src="assetLogoUrl\(selectedPair\.fromAssetSymbol\)" :size="28" \/>/)
  assert.match(swapSource, /<AssetMark :symbol="selectedPair\.toAssetSymbol" :src="assetLogoUrl\(selectedPair\.toAssetSymbol\)" :size="28" \/>/)
  assert.match(swapSource, /logoUrl: assetLogoUrl\(symbol\)/)
  assert.match(swapSource, /<AssetMark :symbol="asset\.symbol" :src="asset\.logoUrl" :size="38" \/>/)
})
