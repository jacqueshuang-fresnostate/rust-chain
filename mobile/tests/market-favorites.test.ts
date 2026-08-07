import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { mapMarketFavorite } from '../src/core/marketFavoriteMapper.ts'
import {
  createMarketFavoritesState,
  type MarketFavoritesApi,
} from '../src/core/marketFavoritesState.ts'
import type { MarketFavorite } from '../src/core/types.ts'

const source = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const apiSource = source('../src/api/marketFavorites.ts')
const storeSource = source('../src/stores/marketFavorites.ts')
const stateSource = source('../src/core/marketFavoritesState.ts')
const appSource = source('../src/App.vue')
const assetMarkSource = source('../src/components/AssetMark.vue')
const tradingApiSource = source('../src/api/trading.ts')
const assetsSource = source('../src/views/AssetsView.vue')
const views = [
  source('../src/views/HomeView.vue'),
  source('../src/views/MarketsView.vue'),
  source('../src/views/TradeView.vue'),
  source('../src/views/MarketDetailView.vue'),
]
const retiredFavoritesKey = ['hippo', 'mobile', 'market', 'favorites'].join('-')

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve
    reject = nextReject
  })
  return { promise, resolve, reject }
}

function favorite(symbol: string, marketId = 1): MarketFavorite {
  return { marketId, symbol }
}

test('自选适配器保留服务端市场标识与三层后台 Logo', () => {
  assert.deepEqual(mapMarketFavorite({
    market_id: 42,
    symbol: 'BTCUSDT',
    logo_url: 'https://cdn.example.test/pairs/btc-usdt.png',
    base_logo_url: 'https://cdn.example.test/assets/btc.png',
    quote_logo_url: 'https://cdn.example.test/assets/usdt.png',
  }), {
    marketId: 42,
    symbol: 'BTCUSDT',
    iconUrl: 'https://cdn.example.test/pairs/btc-usdt.png',
    baseIconUrl: 'https://cdn.example.test/assets/btc.png',
    quoteIconUrl: 'https://cdn.example.test/assets/usdt.png',
  })
  assert.equal(mapMarketFavorite({ market_id: 0, symbol: 'BTCUSDT' }), null)
})

test('单一自选 API 和 Pinia store 覆盖加载、幂等并发与失败回滚', () => {
  assert.match(apiSource, /requestUrl\('\/user\/market-favorites'\)/)
  assert.match(apiSource, /client\.put<[^>]+>\([\s\S]*?favoriteUrl\(symbol\)/)
  assert.match(apiSource, /client\.delete\(favoriteUrl\(symbol\)\)/)
  assert.match(apiSource, /encodeURIComponent\(normalizeSymbol\(symbol\)\)/)

  assert.match(storeSource, /defineStore\('mobile-market-favorites'/)
  assert.match(storeSource, /createMarketFavoritesState\(\{/)
  assert.match(stateSource, /if \(loadPromise\) return loadPromise/)
  assert.match(stateSource, /if \(!normalized \|\| isFavorite\(normalized\) \|\| isPending\(normalized\)\) return false/)
  assert.match(stateSource, /if \(!loaded\.value\) await load\(\)/)
  assert.match(stateSource, /if \(version !== sessionVersion \|\| !loaded\.value\) return false/)
  assert.match(stateSource, /if \(revision === stateRevision\) favorites\.value = next/)
  assert.match(stateSource, /sessionVersion \+= 1[\s\S]*?pendingSymbols\.value = new Set\(\)/)
})

test('store 对同 symbol 去重，并在添加或删除失败后回滚且可重试', async () => {
  const addRequest = deferred<MarketFavorite>()
  const removeRequest = deferred<void>()
  let addCalls = 0
  let removeCalls = 0
  const api: MarketFavoritesApi = {
    fetch: async () => [favorite('BTCUSDT')],
    add: async () => {
      addCalls += 1
      return addRequest.promise
    },
    remove: async () => {
      removeCalls += 1
      return removeRequest.promise
    },
  }
  const state = createMarketFavoritesState(api)
  await state.load()

  const firstAdd = state.add('eth/usdt')
  const duplicateAdd = await state.add('ETHUSDT')
  assert.equal(duplicateAdd, false)
  assert.equal(addCalls, 1)
  assert.equal(state.isFavorite('ETHUSDT'), true)
  assert.equal(state.isPending('ETHUSDT'), true)
  addRequest.reject(new Error('save failed'))
  assert.equal(await firstAdd, false)
  assert.equal(state.isFavorite('ETHUSDT'), false)
  assert.equal(state.isPending('ETHUSDT'), false)

  const firstRemove = state.remove('BTC-USDT')
  const duplicateRemove = await state.remove('BTCUSDT')
  assert.equal(duplicateRemove, false)
  assert.equal(removeCalls, 1)
  assert.equal(state.isFavorite('BTCUSDT'), false)
  removeRequest.reject(new Error('delete failed'))
  assert.equal(await firstRemove, false)
  assert.equal(state.isFavorite('BTCUSDT'), true)
  assert.equal(state.isPending('BTCUSDT'), false)
})

test('store 隔离旧会话响应，且旧 GET 不覆盖并发完成的新 mutation', async () => {
  const oldSessionLoad = deferred<MarketFavorite[]>()
  const newSessionLoad = deferred<MarketFavorite[]>()
  const forceReload = deferred<MarketFavorite[]>()
  let fetchCall = 0
  const api: MarketFavoritesApi = {
    fetch: () => {
      fetchCall += 1
      if (fetchCall === 1) return oldSessionLoad.promise
      if (fetchCall === 2) return newSessionLoad.promise
      return forceReload.promise
    },
    add: async (symbol) => favorite(symbol, 9),
    remove: async () => undefined,
  }
  const state = createMarketFavoritesState(api)

  const staleLoad = state.load()
  state.reset()
  const currentLoad = state.load()
  oldSessionLoad.resolve([favorite('OLDUSDT')])
  await staleLoad
  assert.equal(state.isFavorite('OLDUSDT'), false)
  newSessionLoad.resolve([favorite('BTCUSDT')])
  await currentLoad
  assert.equal(state.isFavorite('BTCUSDT'), true)

  const lateReload = state.load(true)
  assert.equal(await state.add('ETHUSDT'), true)
  forceReload.resolve([favorite('BTCUSDT')])
  await lateReload
  assert.equal(state.isFavorite('BTCUSDT'), true)
  assert.equal(state.isFavorite('ETHUSDT'), true)
})

test('store reset 后忽略旧会话 mutation 的成功响应和 pending 清理', async () => {
  const staleAdd = deferred<MarketFavorite>()
  let fetchCall = 0
  const state = createMarketFavoritesState({
    fetch: async () => fetchCall++ === 0 ? [favorite('BTCUSDT')] : [favorite('SOLUSDT', 2)],
    add: () => staleAdd.promise,
    remove: async () => undefined,
  })
  await state.load()

  const mutation = state.add('ETHUSDT')
  assert.equal(state.isPending('ETHUSDT'), true)
  state.reset()
  await state.load()
  staleAdd.resolve(favorite('ETHUSDT', 3))

  assert.equal(await mutation, false)
  assert.equal(state.isFavorite('SOLUSDT'), true)
  assert.equal(state.isFavorite('ETHUSDT'), false)
  assert.equal(state.pendingSymbols.value.size, 0)
})

test('会话生命周期和四个市场页面共享服务端自选且不再使用旧 localStorage', () => {
  assert.match(appSource, new RegExp('watch\\(\\(\\) => session\\.token[\\s\\S]*?marketFavorites\\.reset\\(\\)[\\s\\S]*?marketFavorites\\.load\\(\\)'))
  for (const view of views) {
    assert.equal(view.includes(retiredFavoritesKey), false)
    assert.doesNotMatch(view, /FAVORITES_STORAGE_KEY|loadFavoriteSymbols/)
    assert.match(view, /useMarketFavoritesStore/)
  }
  assert.match(views[0]!, /marketFavorites\.isFavorite\(ticker\.symbol\)/)
  for (const view of views.slice(1)) {
    assert.match(view, /marketFavorites\.toggle/)
  }
  for (const view of [views[1]!, views[2]!, views[3]!]) {
    assert.match(view, /:aria-pressed=/)
    assert.match(view, /:aria-busy=/)
    assert.match(view, /:disabled=/)
    assert.match(view, /query: \{ redirect:/)
  }
  assert.equal([
    apiSource,
    storeSource,
    stateSource,
    appSource,
    assetMarkSource,
    tradingApiSource,
    assetsSource,
    ...views,
  ].some((contents) => contents.includes(retiredFavoritesKey)), false)
  const contractHeaderStart = views[2]!.indexOf('class="contract-pencil-header"')
  const contractFavoriteStart = views[2]!.indexOf(':class="{ active: isFavorite }"', contractHeaderStart)
  const contractFavorite = views[2]!.slice(
    contractFavoriteStart,
    views[2]!.indexOf('</button>', contractFavoriteStart),
  )
  assert.match(contractFavorite, /:aria-busy="favoriteSaving"/)
  assert.match(contractFavorite, /:disabled="favoriteSaving"/)
  assert.match(views[2]!, /\.contract-header-control \{[\s\S]*?height: 44px;[\s\S]*?width: 44px;/)
  assert.match(views[3]!, /@media \(max-width: 340px\)[\s\S]*?\.market-detail__icon-button \{[\s\S]*?width: 44px;/)
})

test('交易对图片按交易对、基础资产、字母顺序回退且杠杆持仓保留后台图片', () => {
  assert.match(assetMarkSource, /fallbackSrc\?: string/)
  assert.match(assetMarkSource, /\[props\.src, props\.fallbackSrc\]/)
  assert.match(assetMarkSource, /@error="imageIndex \+= 1"/)
  assert.match(assetMarkSource, /v-else aria-hidden="true"/)
  for (const view of views) {
    assert.match(view, /:fallback-src="ticker(?:\.|\?\.)baseIconUrl"/)
  }
  assert.match(tradingApiSource, /logoUrl: String\(wallet\.logo_url \|\| ''\)\.trim\(\) \|\| undefined/)
  assert.match(assetsSource, /logoUrl: row\.spot\?\.logoUrl \|\| row\.margin\?\.logoUrl/)
  assert.match(assetsSource, /<AssetMark :symbol="row\.symbol" :src="row\.logoUrl"/)
})
