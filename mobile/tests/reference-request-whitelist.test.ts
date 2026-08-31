import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (name: string): string => readFileSync(new URL(`../src/api/${name}.ts`, import.meta.url), 'utf8')
const sources = Object.fromEntries(
  ['auth', 'market', 'swap', 'trading', 'seconds', 'earn', 'loan', 'prediction', 'newCoin', 'wallet']
    .map((name) => [name, read(name)]),
) as Record<string, string>

function functionSource(source: string, name: string, next: string): string {
  return source.slice(source.indexOf(`export async function ${name}`), source.indexOf(`export async function ${next}`))
}

test('内存 TTL 只接入明确白名单并在调用点写明 TTL', () => {
  assert.match(sources.auth, /fetchLoginConfig[\s\S]*?5 \* 60_000/)
  assert.match(sources.auth, /fetchRegisterConfig[\s\S]*?5 \* 60_000/)
  assert.match(sources.auth, /fetchCountries[\s\S]*?30 \* 60_000/)
  assert.match(sources.market, /fetchMarketPairs[\s\S]*?2 \* 60_000/)
  assert.match(sources.swap, /fetchConvertPairs[\s\S]*?2 \* 60_000/)
  for (const name of ['trading', 'seconds', 'earn', 'loan']) assert.match(sources[name], /referenceRequestRegistry\.request[\s\S]*?60_000/)
  assert.match(sources.earn, /createReferenceRequestKey\(url, \{[\s\S]*?limit,[\s\S]*?locale: i18n\.global\.locale\.value/)
  assert.match(sources.loan, /createReferenceRequestKey\(url, \{[\s\S]*?limit,[\s\S]*?locale: i18n\.global\.locale\.value/)
  assert.doesNotMatch(sources.seconds, /locale: i18n\.global\.locale\.value/)
  assert.match(sources.prediction, /fetchPredictionConfig[\s\S]*?2 \* 60_000/)
  assert.match(sources.newCoin, /fetchNewCoinProjects[\s\S]*?30_000/)
  assert.equal((sources.wallet.match(/referenceRequestRegistry\.request\(walletReferenceKey/g) || []).length, 3)
  assert.match(sources.wallet, /asset_symbol: normalizedAsset,[\s\S]*?minimum,/)
  assert.match(sources.wallet, /wallet:\$\{readAuthSessionSnapshot\(\)\.scope \|\| 'guest'\}/)
  assert.match(sources.auth, /公开配置不随登录身份变化，缓存键刻意不包含 token/)
})

test('强一致、行情和 mutation 函数不进入 TTL registry', () => {
  const excluded = [
    functionSource(sources.market, 'fetchMarketTickers', 'fetchKlines'),
    functionSource(sources.market, 'fetchKlines', 'fetchOrderBook'),
    functionSource(sources.swap, 'requestConvertQuote', 'confirmConvertQuote'),
    functionSource(sources.seconds, 'fetchSecondsOrders', 'openSecondsOrder'),
    functionSource(sources.prediction, 'requestPredictionQuote', 'confirmPredictionQuote'),
    functionSource(sources.wallet, 'fetchWalletAccounts', 'fetchTodayReturn'),
  ]
  for (const source of excluded) assert.doesNotMatch(source, /referenceRequestRegistry/)
  assert.doesNotMatch(read('client'), /requestCache|localStorage[^\n]*cache|serviceWorker/i)
})
