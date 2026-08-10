import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { computed, ref } from 'vue'
import {
  assetMarkImageSourceAt,
  buildAssetMarkImageSources,
} from '../src/core/assetMark.ts'
import {
  buildSwapAvailableBalanceMap,
  buildSwapPickerAssetLogos,
  ConvertPairContractError,
  mapConvertPair,
  normalizeConvertPairLogoUrl,
  resolveReverseSwapPair,
  resolveSelectedSwapPair,
  resolveSwapPickerPair,
  type ConvertPair,
} from '../src/core/swapAssetLogos.ts'

const swapSource = readFileSync(new URL('../src/views/SwapView.vue', import.meta.url), 'utf8')
const swapApiSource = readFileSync(new URL('../src/api/swap.ts', import.meta.url), 'utf8')
const assetMarkSource = readFileSync(new URL('../src/components/AssetMark.vue', import.meta.url), 'utf8')

function pair(overrides: Partial<ConvertPair> = {}): ConvertPair {
  return {
    id: 1,
    fromAssetId: 10,
    fromAssetSymbol: 'BTC',
    fromAssetLogoUrl: 'https://cdn.example.test/btc.png',
    toAssetId: 20,
    toAssetSymbol: 'USDT',
    toAssetLogoUrl: 'https://cdn.example.test/usdt.png',
    minAmount: 0.001,
    maxAmount: undefined,
    feeRate: 0.001,
    enabled: true,
    ...overrides,
  }
}

test('闪兑交易对适配器执行双方可空 Logo 与 symbol 边界归一化', () => {
  assert.deepEqual(mapConvertPair({
    id: 7,
    from_asset_id: 11,
    from_asset_symbol: ' btc ',
    from_asset_logo_url: '  https://cdn.example.test/btc.png  ',
    to_asset_id: 12,
    to_asset_symbol: ' usdt\n',
    to_asset_logo_url: '   ',
    min_amount: '0.001',
    max_amount: null,
    fee_rate: '0.002',
    enabled: true,
  }), {
    id: 7,
    fromAssetId: 11,
    fromAssetSymbol: 'BTC',
    fromAssetLogoUrl: 'https://cdn.example.test/btc.png',
    toAssetId: 12,
    toAssetSymbol: 'USDT',
    toAssetLogoUrl: undefined,
    minAmount: 0.001,
    maxAmount: undefined,
    feeRate: 0.002,
    enabled: true,
  })

  const missingLogos = mapConvertPair({
    id: 8,
    from_asset_id: 13,
    from_asset_symbol: 'ETH',
    from_asset_logo_url: null,
    to_asset_id: 14,
    to_asset_symbol: 'USDC',
    min_amount: 1,
  })
  assert.equal(missingLogos.fromAssetLogoUrl, undefined)
  assert.equal(missingLogos.toAssetLogoUrl, undefined)
  assert.equal(normalizeConvertPairLogoUrl(undefined), undefined)
  assert.throws(
    () => normalizeConvertPairLogoUrl(42),
    ConvertPairContractError,
  )
  assert.throws(
    () => mapConvertPair({
      id: 9,
      from_asset_id: 15,
      from_asset_symbol: 'ETH',
      from_asset_logo_url: 42 as unknown as string,
      to_asset_id: 16,
      to_asset_symbol: 'USDT',
      min_amount: 1,
    }),
    ConvertPairContractError,
  )
  assert.throws(
    () => mapConvertPair({
      id: 10,
      from_asset_id: 17,
      from_asset_symbol: '   ',
      to_asset_id: 18,
      to_asset_symbol: 'USDT',
      min_amount: 1,
    }),
    ConvertPairContractError,
  )

  assert.match(swapApiSource, /\.map\(mapConvertPair\)\.filter\(\(pair\) => pair\.enabled\)/)
})

test('资产选择器归一化重复 symbol，并按方向保留首个非空交易对 Logo', () => {
  const pairs = [
    {
      fromAssetSymbol: ' btc ',
      toAssetSymbol: ' usdt ',
      fromAssetLogoUrl: ' ',
      toAssetLogoUrl: '   ',
    },
    {
      fromAssetSymbol: 'BTC',
      fromAssetLogoUrl: ' https://cdn.example.test/btc-first.png ',
      toAssetSymbol: 'USDT',
      toAssetLogoUrl: ' https://cdn.example.test/usdt-first.png ',
    },
    {
      fromAssetSymbol: 'btc',
      fromAssetLogoUrl: 'https://cdn.example.test/btc-later.png',
      toAssetSymbol: 'usdt',
      toAssetLogoUrl: 'https://cdn.example.test/usdt-later.png',
    },
    {
      fromAssetSymbol: ' eth ',
      fromAssetLogoUrl: 'https://cdn.example.test/eth.png',
      toAssetSymbol: ' usdc ',
      toAssetLogoUrl: 'https://cdn.example.test/usdc.png',
    },
  ]

  assert.deepEqual(buildSwapPickerAssetLogos(pairs, 'from'), [
    { symbol: 'BTC', logoUrl: 'https://cdn.example.test/btc-first.png' },
    { symbol: 'ETH', logoUrl: 'https://cdn.example.test/eth.png' },
  ])
  assert.deepEqual(buildSwapPickerAssetLogos(pairs, 'to'), [
    { symbol: 'USDT', logoUrl: 'https://cdn.example.test/usdt-first.png' },
    { symbol: 'USDC', logoUrl: 'https://cdn.example.test/usdc.png' },
  ])
})

test('选中交易对、反向交易对及选择器方向切换会响应式更新对应 Logo', () => {
  const pairs = ref<ConvertPair[]>([
    pair(),
    pair({
      id: 2,
      fromAssetId: 20,
      fromAssetSymbol: 'USDT',
      fromAssetLogoUrl: 'https://cdn.example.test/usdt-reverse.png',
      toAssetId: 10,
      toAssetSymbol: 'BTC',
      toAssetLogoUrl: 'https://cdn.example.test/btc-reverse.png',
    }),
    pair({
      id: 3,
      fromAssetId: 30,
      fromAssetSymbol: 'ETH',
      fromAssetLogoUrl: 'https://cdn.example.test/eth-usdc.png',
      toAssetId: 40,
      toAssetSymbol: 'USDC',
      toAssetLogoUrl: 'https://cdn.example.test/usdc.png',
    }),
    pair({
      id: 4,
      fromAssetId: 30,
      fromAssetSymbol: 'ETH',
      fromAssetLogoUrl: 'https://cdn.example.test/eth-usdt.png',
      toAssetId: 20,
      toAssetSymbol: 'USDT',
      toAssetLogoUrl: 'https://cdn.example.test/usdt-eth.png',
    }),
  ])
  const pairId = ref(1)
  const pickerSide = ref<'from' | 'to'>('from')
  const selected = computed(() => resolveSelectedSwapPair(pairs.value, pairId.value))
  const pickerAssets = computed(() => buildSwapPickerAssetLogos(pairs.value, pickerSide.value))

  assert.equal(selected.value?.fromAssetLogoUrl, 'https://cdn.example.test/btc.png')
  assert.equal(selected.value?.toAssetLogoUrl, 'https://cdn.example.test/usdt.png')
  assert.equal(pickerAssets.value[0]?.symbol, 'BTC')

  const reversed = resolveReverseSwapPair(pairs.value, selected.value!)
  assert.equal(reversed?.id, 2)
  pairId.value = reversed!.id
  assert.equal(selected.value?.fromAssetLogoUrl, 'https://cdn.example.test/usdt-reverse.png')
  assert.equal(selected.value?.toAssetLogoUrl, 'https://cdn.example.test/btc-reverse.png')

  pickerSide.value = 'to'
  assert.equal(pickerAssets.value[0]?.symbol, 'USDT')
  assert.equal(pickerAssets.value[0]?.logoUrl, 'https://cdn.example.test/usdt.png')

  const preservingCounterAsset = resolveSwapPickerPair(pairs.value, 'from', ' eth ', pairs.value[0])
  assert.equal(preservingCounterAsset?.id, 4)
  assert.equal(preservingCounterAsset?.fromAssetLogoUrl, 'https://cdn.example.test/eth-usdt.png')
  pairId.value = preservingCounterAsset!.id
  assert.equal(selected.value?.fromAssetLogoUrl, 'https://cdn.example.test/eth-usdt.png')
  assert.equal(selected.value?.toAssetLogoUrl, 'https://cdn.example.test/usdt-eth.png')
})

test('AssetMark 对 null、空白和连续图片失败执行字母回退', () => {
  assert.deepEqual(buildAssetMarkImageSources(null, undefined, '  '), [])
  const sources = buildAssetMarkImageSources(
    ' https://cdn.example.test/primary.png ',
    'https://cdn.example.test/primary.png',
    ' https://cdn.example.test/fallback.png ',
  )
  assert.deepEqual(sources, [
    'https://cdn.example.test/primary.png',
    'https://cdn.example.test/fallback.png',
  ])
  assert.equal(assetMarkImageSourceAt(sources, 0), 'https://cdn.example.test/primary.png')
  assert.equal(assetMarkImageSourceAt(sources, 1), 'https://cdn.example.test/fallback.png')
  assert.equal(assetMarkImageSourceAt(sources, 2), undefined)

  assert.match(assetMarkSource, /buildAssetMarkImageSources\(props\.src, props\.fallbackSrc\)/)
  assert.match(assetMarkSource, /assetMarkImageSourceAt\(imageSources\.value, imageIndex\.value\)/)
  assert.match(assetMarkSource, /@error="imageIndex \+= 1"/)
  assert.match(assetMarkSource, /<b v-else aria-hidden="true">\{\{ initial \}\}<\/b>/)
})

test('闪兑主卡片与选择器只用交易对 Logo，钱包账户只提供余额', () => {
  const balances = buildSwapAvailableBalanceMap([
    {
      symbol: ' btc ',
      available: 1.25,
      logoUrl: 'https://wallet.example.test/must-not-be-consumed.png',
    },
  ] as Array<{ symbol: string; available: number; logoUrl: string }>)
  assert.deepEqual([...balances], [['BTC', 1.25]])

  assert.match(swapSource, /const availableBySymbol = computed\(\(\) => buildSwapAvailableBalanceMap\(accounts\.value\)\)/)
  assert.match(swapSource, /const availableBalance = \(symbol: string\): number => availableBySymbol\.value\.get\(symbol\.trim\(\)\.toUpperCase\(\)\) \|\| 0/)
  assert.match(swapSource, /buildSwapPickerAssetLogos\(pairs\.value, pickerSide\.value\)/)
  assert.match(swapSource, /balance: availableBalance\(asset\.symbol\)/)
  assert.match(swapSource, /<AssetMark :symbol="selectedPair\.fromAssetSymbol" :src="selectedPair\.fromAssetLogoUrl" :size="28" \/>/)
  assert.match(swapSource, /<AssetMark :symbol="selectedPair\.toAssetSymbol" :src="selectedPair\.toAssetLogoUrl" :size="28" \/>/)
  assert.match(swapSource, /<AssetMark :symbol="asset\.symbol" :src="asset\.logoUrl" :size="38" \/>/)
  assert.doesNotMatch(swapSource, /assetLogoUrl/)
  assert.doesNotMatch(swapSource, /account[^\n]*\.logoUrl/)
})
