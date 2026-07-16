import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')

function read(relativePath: string) {
  return readFileSync(resolve(root, relativePath), 'utf8')
}

test('prediction page exposes polymarket-style list and detail routing', () => {
  const routerSource = read('src/router/index.ts')
  const predictionSource = read('src/views/Prediction.vue')
  const predictionOrdersSource = read('src/views/User/PredictionOrders.vue')
  const i18nSource = read('src/i18n/index.ts')

  assert.match(routerSource, /path:\s*'prediction\/:id'/)
  assert.match(routerSource, /name:\s*'PredictionDetail'/)
  assert.match(predictionSource, /fetchPredictionMarket/)
  assert.match(predictionSource, /name:\s*'PredictionDetail'/)
  assert.match(predictionSource, /const isDetailView = computed/)
  assert.match(predictionSource, /const topicFilters = computed/)
  assert.match(predictionSource, /const relatedMarkets = computed/)
  assert.match(predictionSource, /getWallets/)
  assert.match(predictionSource, /const stakeAssetOptions = computed/)
  assert.match(predictionSource, /const filteredStakeAssetOptions = computed/)
  assert.match(predictionSource, /selectedAssetBalanceText/)
  assert.match(predictionSource, /const maxStakeByPayoutCap = computed/)
  assert.match(predictionSource, /const maxStakeAmount = computed/)
  assert.match(predictionSource, /market\?\.payout_cap_overrides_json/)
  assert.match(predictionSource, /optionalNumberValue\(market\?\.payout_cap_overrides_json/)
  assert.match(predictionSource, /stakeAmountError/)
  assert.match(predictionSource, /toStakeInputAmount\(maxStakeAmount\.value\)/)
  assert.match(predictionSource, /function formatPayoutCapAmount/)
  assert.match(predictionSource, /formatPayoutCapAmount\(quote\.effective_payout_cap, quote\.asset_symbol\)/)
  assert.match(predictionOrdersSource, /function formatPayoutCap/)
  assert.match(predictionOrdersSource, /formatPayoutCap\(order\.effective_payout_cap, order\.asset_symbol\)/)
  assert.match(predictionOrdersSource, /<InfoItem :label="t\('prediction\.market'\)" :value="order\.market_title" \/>/)
  assert.doesNotMatch(predictionOrdersSource, /<th[^>]*>\{\{ t\('prediction\.market'\) \}\}<\/th>/)
  assert.doesNotMatch(predictionOrdersSource, /<button[^>]*>\s*\{\{ order\.market_title \}\}/)
  assert.match(predictionSource, /import \{ readAuthToken \} from '@\/utils\/authStorage'/)
  assert.match(predictionSource, /const hasAuthSession = computed\(\(\) => Boolean\(readAuthToken\(\)\)\)/)
  assert.match(predictionSource, /:src="selectedStakeAsset\?\.logoUrl"/)
  assert.doesNotMatch(predictionSource, /<select v-model="assetId"/)
  assert.match(predictionSource, /back_to_markets/)
  assert.match(i18nSource, /browse_trending:\s*'Trending'/)
  assert.match(i18nSource, /browse_trending:\s*'趋势'/)
  assert.match(i18nSource, /detail_not_found:\s*'Market unavailable'/)
  assert.match(i18nSource, /detail_not_found:\s*'市场不可用'/)
  assert.match(i18nSource, /available_balance:\s*'Available'/)
  assert.match(i18nSource, /available_balance:\s*'可用'/)
  assert.match(i18nSource, /amount_exceeds_payout_cap:\s*'Stake amount is too high/)
  assert.match(i18nSource, /amount_exceeds_payout_cap:\s*'该市场下注金额过高/)
})

test('prediction quote and order api uses shared authenticated request instance', () => {
  const apiSource = read('src/api/prediction.ts')

  assert.match(apiSource, /request\.instance\.post<PredictionQuote>\(backendApiUrl\('\/prediction\/quotes'\)/)
  assert.match(apiSource, /request\.instance\.post<\{ order: Omit<PredictionOrder, 'orderNo'>; changed: boolean \}>\(backendApiUrl\('\/prediction\/orders'\)/)
  assert.doesNotMatch(apiSource, /fetch\(['"`].*\/prediction\/quotes/)
})
