import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const zone = read('../src/views/NewCoinsView.vue')
const detail = read('../src/views/NewCoinDetailView.vue')
const records = read('../src/views/NewCoinRecordsView.vue')
const projectCard = read('../src/components/new-coin/NewCoinProjectCard.vue')
const opportunityCard = read('../src/components/new-coin/NewCoinOpportunityCard.vue')
const recordCard = read('../src/components/new-coin/NewCoinRecordCard.vue')
const api = read('../src/api/newCoin.ts')
const marketApi = read('../src/api/market.ts')
const walletApi = read('../src/api/wallet.ts')
const model = read('../src/core/newCoinModel.ts')
const presentation = read('../src/core/newCoinPresentation.ts')
const selectedCss = read('../src/styles/pencil-selected-pages.css')
const backendRepository = read('../../src/modules/new_coin/infrastructure.rs')
const backendPresentation = read('../../src/modules/new_coin/presentation.rs')

test('all eight selected frames and the exact tracked banner are owned by production', () => {
  assert.match(zone, /data-pencil-source="oOJ0q ZTtvY XG67j E2qzxN"/)
  assert.match(detail, /data-pencil-source="nFwYy B6Qh9J"/)
  assert.match(records, /data-pencil-source="A9It6g h4gfd"/)
  assert.match(zone, /@\/assets\/new-coin-launch-banner\.jpg/)
  assert.doesNotMatch(`${zone}\n${detail}\n${records}`, /mobile\/pencil|\.pen\b/)

  const banner = readFileSync(new URL('../src/assets/new-coin-launch-banner.jpg', import.meta.url))
  assert.equal(createHash('sha256').update(banner).digest('hex'), 'becbd0fbbc86c66082a163f2358fe1878f790b4591a34fc4fb84ab857e1cdc36')
})

test('zone and opportunities use exact filters, shared market lease, and authoritative pair navigation', () => {
  assert.match(zone, /type PrimaryTab = 'activities' \| 'opportunities'/)
  for (const lifecycle of ['preheat', 'subscription', 'distribution', 'listed']) {
    assert.match(zone, new RegExp(`key: '${lifecycle}'`))
  }
  for (const filter of ['upcoming', 'listedToday', 'hotGains']) {
    assert.match(zone, new RegExp(`key: '${filter}'`))
  }
  assert.match(zone, /useMarketStore\(\)/)
  assert.match(zone, /market\.refresh\(\)/)
  assert.match(zone, /market\.startLiveUpdates\(MARKET_CONSUMER_ID\)/)
  assert.match(zone, /market\.stopLiveUpdates\(MARKET_CONSUMER_ID\)/)
  assert.doesNotMatch(zone, /fetchMarketTickers|setInterval\([^]*?market\.refresh/)
  assert.match(presentation, /tickerById\.get\(project\.postListingPairId\)/)
  assert.doesNotMatch(presentation, /project\.postListingPurchaseEnabled \|\| !project\.postListingPairId/)
  assert.doesNotMatch(presentation, /normalizeSymbol|tickerFor/)
  assert.match(zone, /params: \{ symbol: opportunity\.ticker\.symbol\.replace\('\/', '_'\) \}/)
})

test('guest zone and opportunities stay public while account and record reads remain guarded', () => {
  assert.doesNotMatch(
    zone,
    /fetchNewCoin(?:Subscriptions|Distributions|Purchases|Unlocks)|fetchWalletAccounts/,
  )
  assert.match(zone, /function initializeMarket()[\s\S]*?market\.refresh\(\)/)
  assert.match(api, /publicApiRequestConfig\(\{ params: \{ limit \} \}\)/)
  assert.match(api, /fetchNewCoinProject[\s\S]*?publicApiRequestConfig\(\)/)
  assert.match(marketApi, /fetchMarketTickers[\s\S]*?publicApiRequestConfig\(\)/)
  assert.match(detail, /session\.isAuthenticated \? await fetchWalletAccounts\(\) : \[\]/)
  assert.match(records, /async function load\(\): Promise<void> \{\s*if \(!session\.isAuthenticated\) return/)
  assert.doesNotMatch(api.slice(api.indexOf('export async function subscribeNewCoin')), /publicApiRequestConfig/)
  assert.doesNotMatch(walletApi, /publicApiRequestConfig/)
})

test('detail uses the project quote asset and preserves exact-decimal mutations and dialog behavior', () => {
  assert.match(detail, /account\.assetId === project\.value\?\.quoteAssetId/)
  assert.doesNotMatch(detail, /symbol === 'USDT'|accounts\.value\[0\]|<select/)
  assert.match(detail, /decimalPortion\(availableText\.value, percentage, 100, 18\)/)
  assert.match(detail, /quoteAssetId: project\.value\.quoteAssetId/)
  assert.match(detail, /pairId: project\.value\.postListingPairId/)
  assert.match(detail, /const executionPriceText = computed<DecimalText \| null>\(\(\) => project\.value\?\.issuePriceText \|\| null\)/)
  assert.match(detail, /price: project\.value\.issuePriceText/)
  assert.match(detail, /useModalDialog\(reviewOpen, reviewDialog/)
  assert.match(detail, /<Teleport to="body">[\s\S]*?new-coin-detail-review-layer/)
  assert.match(detail, /role="dialog"/)
  assert.match(detail, /aria-modal="true"/)
  assert.match(detail, /@click\.self="closeReview"/)
  assert.match(detail, /trapReviewFocus\(event, closeReview\)/)
  assert.doesNotMatch(detail, /selectedTicker|useMarketStore|fetchMarketTickers/)
  assert.match(backendRepository, /price does not match the server-authoritative issue price/)
})

test('records merge four sources chronologically and retain typed filtering and unlock actions', () => {
  for (const request of [
    'fetchNewCoinProjects',
    'fetchNewCoinSubscriptions',
    'fetchNewCoinDistributions',
    'fetchNewCoinPurchases',
    'fetchNewCoinUnlocks',
  ]) {
    assert.match(records, new RegExp(`${request}\\(\\)`))
  }
  assert.match(records, /buildUnifiedNewCoinRecords\(/)
  assert.match(records, /filterUnifiedNewCoinRecords\(/)
  assert.match(records, /SlidersHorizontal/)
  assert.match(records, /typeSheetOpen/)
  assert.match(records, /await payNewCoinUnlockFee\(/)
  assert.match(records, /await releaseNewCoinUnlock\(/)
  assert.match(records, /useModalDialog\(/)
  assert.match(records, /<Teleport to="body">[\s\S]*?new-coin-record-dialog-layer/)
  assert.match(recordCard, /height: 168px/)
  assert.match(recordCard, /AssetMark[^>]*?:symbol="symbol" :src="record\.assetLogoUrl"/)
  assert.match(recordCard, /props\.record\.distribution\.lockPositionId[\s\S]*?newCoin\.lockPositionNumber/)
})

test('selected geometry, responsive containment, and dark palettes remain route scoped', () => {
  assert.match(zone, /height: 54px[\s\S]*?height: 148px[\s\S]*?height: 50px[\s\S]*?height: 36px/)
  assert.match(zone, /\.new-coins-project-content \{\s*padding: 8px 16px 18px;\s*\}/)
  assert.match(zone, /\.new-coins-project-content > h2 \{[\s\S]*?height: 36px;[\s\S]*?margin: 0 0 12px;/)
  assert.match(projectCard, /\.new-coin-project-card \{[\s\S]*?border-radius: 22px;[\s\S]*?height: 300px/)
  assert.match(opportunityCard, /\.new-coin-opportunity-card \{[\s\S]*?border-radius: 18px;[\s\S]*?height: 140px/)
  assert.match(detail, /height: 56px[\s\S]*?height: 210px[\s\S]*?height: 112px[\s\S]*?height: 104px[\s\S]*?min-height: 328px/)
  assert.match(records, /height: 58px[\s\S]*?height: 56px/)
  assert.match(selectedCss, /html\[data-theme='dark'\] \.new-coins-pencil/)
  assert.match(selectedCss, /html\[data-theme='dark'\] \.new-coin-detail-pencil/)
  assert.match(selectedCss, /html\[data-theme='dark'\] \.new-coin-records-page/)
  for (const source of [zone, detail, records]) {
    assert.match(source, /overflow-x: clip/)
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /env\(safe-area-inset-bottom\)/)
    assert.doesNotMatch(source, /https?:\/\//)
  }
})

test('supported unlock copy, truthful project-name fallback, long values, and record hues stay explicit', () => {
  for (const unlockType of ['immediate_on_listing', 'fixed_time', 'relative_period']) {
    assert.match(presentation, new RegExp(`${unlockType}: 'newCoin\\.`))
  }
  assert.match(projectCard, /newCoinUnlockTypeTranslationKey\(props\.project\.unlockType\)/)
  assert.match(detail, /newCoinUnlockTypeTranslationKey\(type\)/)
  assert.match(projectCard, /props\.project\.name \|\| t\('newCoin\.projectNameUnavailable'\)/)
  assert.match(opportunityCard, /project\.value\.name \|\| t\('newCoin\.projectNameUnavailable'\)/)
  assert.match(detail, /project\.value\?\.name \|\| t\('newCoin\.projectNameUnavailable'\)/)
  assert.match(recordCard, /props\.record\.project\.name \|\| t\('newCoin\.projectNameUnavailable'\)/)
  assert.match(projectCard, /:title="totalSupplyDisplay"/)
  assert.match(projectCard, /\.is-long-value/)
  assert.match(recordCard, /background: var\(--new-coin-record-rail\)/)
  assert.match(recordCard, /first-child dd \{\s*color: var\(--new-coin-record-active\)/)
  assert.match(recordCard, /asset-mark--fallback[\s\S]*?--asset-color: var\(--new-coin-record-active\)/)
  assert.doesNotMatch(`${recordCard}\n${selectedCss}`, /new-coin-record-(?:pending|complete)/)
})

test('mobile and backend contracts carry exact supply and authoritative asset metadata', () => {
  for (const field of [
    'totalSupplyText',
    'reservedSupplyText',
    'allocatedSupplyText',
    'remainingSupplyText',
    'quoteAssetId',
    'quoteAssetSymbol',
    'quoteAssetLogoUrl',
  ]) {
    assert.match(model, new RegExp(field))
  }
  assert.match(model, /typeof value !== 'string'/)
  assert.match(api, /mapNewCoinProject/)
  assert.doesNotMatch(api, /asNumber/)
  assert.match(projectCard, /<AssetMark :symbol="project\.symbol" :src="project\.logoUrl" :size="40" \/>/)
  assert.match(projectCard, /new-coin-project-card__issue-price-value">\{\{ issuePrice \}\}/)
  assert.match(projectCard, /new-coin-project-card__issue-price-symbol">\{\{ quoteSymbol \}\}/)
  assert.match(projectCard, /\.new-coin-project-card__issue-price-symbol \{[\s\S]*?flex: 0 0 auto;/)
  assert.doesNotMatch(projectCard, /\{\{ issuePriceDisplay \}\}/)
  assert.match(backendRepository, /LEFT JOIN assets AS project_asset ON project_asset\.id = projects\.asset_id/)
  assert.match(backendRepository, /LEFT JOIN assets AS quote_asset ON quote_asset\.id = projects\.quote_asset_id/)
  for (const field of ['name', 'logo_url', 'quote_asset_symbol', 'quote_asset_logo_url']) {
    assert.match(backendPresentation, new RegExp(`pub\\(crate\\) ${field}: Option<String>`))
  }
})

test('new-coin locale trees are symmetric', () => {
  assert.deepEqual(flattenKeys(zhCN.newCoin), flattenKeys(en.newCoin))
})

function flattenKeys(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object') return [prefix]
  return Object.entries(value as Record<string, unknown>)
    .flatMap(([key, entry]) => flattenKeys(entry, prefix ? `${prefix}.${key}` : key))
    .sort()
}
