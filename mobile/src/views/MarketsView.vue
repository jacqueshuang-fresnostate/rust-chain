<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { RefreshCw, Search, SlidersHorizontal, Star } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { fetchKlines } from '@/api/market'
import { formatCompact, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const marketFavorites = useMarketFavoritesStore()
const navigation = useNavigationStore()
const session = useSessionStore()
const { t } = useI18n()

const query = ref('')
type MarketCategory = 'popular' | 'favorites' | 'spot' | 'contract' | 'gainers'
const category = ref<MarketCategory>('popular')
const sparklineCloses = ref<Record<string, number[]>>({})
const neutralSparklinePoints = '0,17 76,17'
let sparklineRequestId = 0

const categories = computed(() => [
  { key: 'popular' as const, label: t('markets.popular') },
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'spot' as const, label: t('trade.spot') },
  { key: 'contract' as const, label: t('trade.contract') },
  { key: 'gainers' as const, label: t('markets.gainers') },
])

const pickerMode = computed(() => route.query.purpose === 'trade')
const title = computed(() => pickerMode.value ? t('markets.pickerTitle') : t('markets.title'))
const marketRowsUnavailable = computed(() => (
  marketStore.error
  || (!marketStore.updatedAt && !marketStore.tickers.length)
))

const rows = computed(() => {
  const source = [...marketStore.tickers]
  const keyword = query.value.trim().toUpperCase()
  let filtered = keyword ? source.filter((item) => item.symbol.includes(keyword)) : source
  if (category.value === 'favorites') {
    filtered = filtered.filter((item) => marketFavorites.isFavorite(item.symbol))
  }
  if (category.value === 'gainers') return filtered.sort((left, right) => right.changePercent - left.changePercent)
  if (category.value === 'popular') return filtered.sort((left, right) => right.volume - left.volume)
  return filtered
})

const positiveRate = computed(() => rows.value.length
  ? (rows.value.filter((item) => item.changePercent >= 0).length / rows.value.length) * 100
  : 0)
const marketTemperature = computed(() => Math.round(positiveRate.value))
const hasTemperatureSample = computed(() => !marketRowsUnavailable.value && rows.value.length > 0)

function openMarket(symbol: string): void {
  const routeSymbol = symbol.replace('/', '_')
  if (pickerMode.value) {
    navigation.rememberTradeSymbol(routeSymbol)
    const mode = route.query.mode === 'contract' ? 'contract' : 'spot'
    navigation.rememberTradeMode(mode)
    void router.replace({
      name: 'trade',
      params: { symbol: routeSymbol },
      query: mode === 'contract' ? { mode } : undefined,
    })
    return
  }
  void router.push({ name: 'market-detail', params: { symbol: routeSymbol } })
}

function toggleFavorite(symbol: string): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: route.fullPath } })
    return
  }
  void marketFavorites.toggle(symbol)
}

function cycleCategory(): void {
  const index = categories.value.findIndex((item) => item.key === category.value)
  category.value = categories.value[(index + 1) % categories.value.length]!.key
}

function sparklineTone(symbol: string): 'positive' | 'negative' | 'neutral' {
  const values = sparklineCloses.value[symbol] || []
  if (values.length < 2) return 'neutral'
  const delta = values[values.length - 1]! - values[0]!
  if (delta > 0) return 'positive'
  if (delta < 0) return 'negative'
  return 'neutral'
}

function sparklinePoints(symbol: string): string {
  const values = sparklineCloses.value[symbol] || []
  if (values.length < 2) return neutralSparklinePoints
  const maximum = Math.max(...values)
  const minimum = Math.min(...values)
  if (maximum === minimum) return neutralSparklinePoints
  return values.map((value, index) => {
    const x = (index / (values.length - 1)) * 76
    const y = 30 - ((value - minimum) / (maximum - minimum)) * 24
    return `${x},${y}`
  }).join(' ')
}

async function loadSparklines(): Promise<void> {
  const requestId = ++sparklineRequestId
  const symbols = [...new Set(marketStore.tickers.map((ticker) => ticker.symbol))]
  if (marketStore.error || !symbols.length) {
    sparklineCloses.value = {}
    return
  }

  const results = await Promise.allSettled(
    symbols.map((symbol) => fetchKlines(symbol, '15m', 24)),
  )
  if (requestId !== sparklineRequestId) return

  const next: Record<string, number[]> = {}
  symbols.forEach((symbol, index) => {
    const result = results[index]
    next[symbol] = result?.status === 'fulfilled'
      ? result.value
        .map((point) => point.close)
        .filter((close) => Number.isFinite(close) && close > 0)
      : []
  })
  sparklineCloses.value = next
}

async function refreshMarkets(force = false): Promise<void> {
  await marketStore.refresh(force)
  marketStore.startLiveUpdates()
  await loadSparklines()
}

onMounted(async () => {
  await refreshMarkets()
})
onUnmounted(() => {
  sparklineRequestId += 1
  marketStore.stopLiveUpdates()
})
</script>

<template>
  <main v-if="pickerMode" class="page markets-picker-page">
    <PageHeader
      :title="title"
      :eyebrow="t('markets.overview')"
      :back="true"
      :fallback="navigation.lastTradePath"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('markets.refresh')"
          :disabled="marketStore.loading"
          @click="refreshMarkets(true)"
        >
          <RefreshCw :size="20" :class="{ spin: marketStore.loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content">
      <label class="input market-picker-search">
        <Search :size="18" aria-hidden="true" />
        <input v-model="query" type="search" :placeholder="t('markets.searchPlaceholder')" />
      </label>
      <div
        class="market-picker-list"
        :class="{ 'market-picker-list--reserved': marketRowsUnavailable }"
        :aria-busy="marketRowsUnavailable && !marketStore.error"
        :aria-label="marketRowsUnavailable ? t(marketStore.error ? 'common.marketLoadFailed' : 'common.loading') : undefined"
      >
        <template v-if="marketRowsUnavailable">
          <div v-for="row in 5" :key="`market-picker-skeleton-${row}`" class="market-picker-skeleton-row" aria-hidden="true">
            <span class="market-picker-skeleton-mark root-skeleton" />
            <span class="market-picker-skeleton-copy">
              <i class="market-picker-skeleton-line root-skeleton" />
              <i class="market-picker-skeleton-line compact root-skeleton" />
            </span>
            <i class="market-picker-skeleton-price root-skeleton" />
          </div>
          <div v-if="marketStore.error" class="market-picker-state" role="alert">
            <span>{{ t('common.marketLoadFailed') }}</span>
            <button type="button" :disabled="marketStore.loading" @click="refreshMarkets(true)">{{ t('common.retry') }}</button>
          </div>
        </template>

        <template v-else>
          <button
            v-for="ticker in rows"
            :key="ticker.symbol"
            type="button"
            @click="openMarket(ticker.symbol)"
          >
            <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :fallback-src="ticker.baseIconUrl" :size="34" />
            <span><strong>{{ ticker.symbol }}</strong><small>{{ t('markets.volume', { value: formatCompact(ticker.volume) }) }}</small></span>
            <b>{{ formatPrice(ticker.lastPrice) }}</b>
          </button>
          <div v-if="!rows.length" class="empty-state">
            {{ t('markets.noResults') }}
          </div>
        </template>
      </div>
    </div>
  </main>

  <main v-else class="view markets-view prototype-root-view">
    <header class="page-intro markets-hero">
      <span class="eyebrow">{{ t('rootPrototype.marketPulse') }}</span>
      <h1>{{ t('rootPrototype.marketHeadlineLine1') }}<br />{{ t('rootPrototype.marketHeadlineLine2') }}</h1>
    </header>

    <section class="market-controls" :aria-label="t('markets.overview')">
      <label class="search-field">
        <Search :size="18" aria-hidden="true" />
        <input v-model="query" type="search" :placeholder="t('markets.searchPlaceholder')" />
        <button class="inline-icon" type="button" :aria-label="t('rootPrototype.cycleMarketFilter')" @click="cycleCategory">
          <SlidersHorizontal :size="17" aria-hidden="true" />
        </button>
      </label>

      <div class="filter-rail" role="tablist" :aria-label="t('markets.overview')">
        <button
          v-for="item in categories"
          :key="item.key"
          type="button"
          role="tab"
          :aria-selected="category === item.key"
          :class="{ active: category === item.key }"
          @click="category = item.key"
        >
          {{ item.label }}
        </button>
      </div>
    </section>

    <section class="market-index" :aria-label="t('rootPrototype.marketTemperature')">
      <div class="market-index__summary">
        <span>{{ t('rootPrototype.marketTemperature') }}</span>
        <strong v-if="hasTemperatureSample" class="numeric">
          {{ marketTemperature }}<small>%</small>
        </strong>
      </div>
      <div class="temperature-line"><i :style="{ width: `${hasTemperatureSample ? marketTemperature : 0}%` }" /></div>
      <span class="market-index__state">
        {{ t(hasTemperatureSample ? 'rootPrototype.marketStrong' : 'rootPrototype.marketNoSample') }}
      </span>
    </section>

    <div class="market-table-head" aria-hidden="true">
      <span>{{ t('rootPrototype.pairAndVolume') }}</span>
      <span>{{ t('rootPrototype.trend') }}</span>
      <span>{{ t('rootPrototype.priceAndChange') }}</span>
    </div>

    <div
      class="market-list"
      :class="{ 'root-market-reserved': marketRowsUnavailable }"
      :aria-busy="marketRowsUnavailable && !marketStore.error"
      :aria-label="marketRowsUnavailable ? t(marketStore.error ? 'common.marketLoadFailed' : 'common.loading') : undefined"
    >
      <template v-if="marketRowsUnavailable">
        <article v-for="row in 5" :key="`market-skeleton-${row}`" class="market-row market-row--skeleton" aria-hidden="true">
          <span class="favorite-button"><i class="root-market-skeleton-dot root-skeleton" /></span>
          <span class="market-main">
            <span class="coin-orbit root-skeleton" />
            <span class="market-name">
              <i class="root-market-skeleton-block root-skeleton" />
              <i class="root-market-skeleton-block compact root-skeleton" />
            </span>
            <span class="sparkline root-skeleton" />
            <span class="market-price">
              <i class="root-market-skeleton-block root-skeleton" />
              <i class="root-market-skeleton-block compact root-skeleton" />
            </span>
          </span>
        </article>
        <div v-if="marketStore.error" class="root-market-reserved-state" role="alert">
          <span>{{ t('common.marketLoadFailed') }}</span>
          <button type="button" :disabled="marketStore.loading" @click="refreshMarkets(true)">{{ t('common.retry') }}</button>
        </div>
      </template>

      <template v-else>
        <article v-for="ticker in rows" :key="ticker.symbol" class="market-row">
          <button
            class="favorite-button"
            :class="{ active: marketFavorites.isFavorite(ticker.symbol) }"
            type="button"
            :aria-label="t(marketFavorites.isFavorite(ticker.symbol) ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: ticker.symbol })"
            :aria-pressed="marketFavorites.isFavorite(ticker.symbol)"
            :aria-busy="marketFavorites.isPending(ticker.symbol)"
            :disabled="marketFavorites.isPending(ticker.symbol)"
            @click="toggleFavorite(ticker.symbol)"
          >
            <Star :size="14" :fill="marketFavorites.isFavorite(ticker.symbol) ? 'currentColor' : 'none'" />
          </button>
          <button class="market-main" type="button" @click="openMarket(ticker.symbol)">
            <span class="coin-orbit">
              <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :fallback-src="ticker.baseIconUrl" :size="32" />
            </span>
            <span class="market-name">
              <strong>{{ ticker.symbol }}</strong>
              <small>{{ t('markets.volume', { value: formatCompact(ticker.volume) }) }}</small>
            </span>
            <svg
              class="sparkline"
              :class="sparklineTone(ticker.symbol)"
              viewBox="0 0 76 34"
              aria-hidden="true"
            >
              <polyline
                :points="sparklinePoints(ticker.symbol)"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                vector-effect="non-scaling-stroke"
              />
            </svg>
            <span class="market-price">
              <strong class="numeric">{{ formatPrice(ticker.lastPrice) }}</strong>
              <small :class="ticker.changePercent >= 0 ? 'positive' : 'negative'">
                {{ formatPercent(ticker.changePercent) }}
              </small>
            </span>
          </button>
        </article>

        <div v-if="!rows.length" class="empty-state">
          <Search :size="24" aria-hidden="true" />
          <strong>{{ t('markets.noResults') }}</strong>
          <span>{{ t('rootPrototype.noMarketHint') }}</span>
        </div>
      </template>
    </div>
  </main>
</template>
