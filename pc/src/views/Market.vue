<template>
  <div class="h-full overflow-y-auto bg-background text-foreground">
    <div class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-6 lg:px-6">
      <section class="flex flex-col gap-4 rounded-xl border border-border bg-card/70 p-5 shadow-lg shadow-black/10 md:flex-row md:items-end md:justify-between">
        <div>
          <p class="mb-2 text-xs font-bold uppercase tracking-[0.2em] text-primary">{{ t('market.overview') }}</p>
          <h1 class="text-3xl font-black tracking-tight md:text-4xl">{{ t('market.overview_title') }}</h1>
          <p class="mt-2 max-w-2xl text-sm text-muted-foreground">{{ t('market.overview_subtitle') }}</p>
        </div>
        <div class="relative w-full md:w-80">
          <Icon icon="lucide:search" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            v-model="searchText"
            type="search"
            :placeholder="t('market.search_placeholder')"
            class="h-11 w-full rounded-lg border border-input bg-background/70 pl-10 pr-4 text-sm outline-none transition-colors focus:border-primary"
          />
        </div>
      </section>

      <section class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <article
          v-for="card in overviewCards"
          :key="card.key"
          class="rounded-xl border border-border bg-card/70 p-4 shadow-lg shadow-black/5"
        >
          <div class="mb-3 flex items-center justify-between">
            <div>
              <p class="text-sm font-bold">{{ card.title }}</p>
              <p class="text-xs text-muted-foreground">{{ card.subtitle }}</p>
            </div>
            <Icon :icon="card.icon" class="h-5 w-5 text-primary" />
          </div>
          <div class="space-y-1">
            <button
              v-for="ticker in card.items"
              :key="`${card.key}-${ticker.symbol}`"
              type="button"
              class="flex w-full items-center justify-between rounded-lg px-2 py-2 text-left transition-colors hover:bg-muted/70"
              @click="goToTrade(ticker.symbol)"
            >
              <span class="flex min-w-0 items-center gap-2">
                <PairLogo class="h-7 w-7 shrink-0" :symbol="ticker.symbol" :src="ticker.icon" />
                <span class="min-w-0">
                  <span class="block truncate text-sm font-bold">{{ ticker.symbol }}</span>
                  <span class="block text-xs text-muted-foreground">{{ formatNumber(ticker.close, 'price') }}</span>
                </span>
              </span>
              <span class="text-right text-xs font-bold" :class="changeClass(ticker.chg)">
                {{ formatSignedChange(ticker.chg) }}
              </span>
            </button>
            <div v-if="card.items.length === 0" class="rounded-lg border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
              {{ t('market.no_markets') }}
            </div>
          </div>
        </article>
      </section>

      <section class="rounded-xl border border-border bg-card/70 shadow-xl shadow-black/10">
        <div class="flex flex-col gap-4 border-b border-border p-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="flex flex-wrap gap-2">
              <button
                v-for="tab in marketTabs"
                :key="tab.key"
                type="button"
                class="rounded-lg px-4 py-2 text-sm font-bold transition-colors"
                :class="activeTab === tab.key ? 'bg-primary text-primary-foreground shadow-lg shadow-primary/20' : 'bg-muted/60 text-muted-foreground hover:text-foreground'"
                @click="activeTab = tab.key"
              >
                {{ t(tab.labelKey) }}
              </button>
            </div>
            <div class="flex items-center gap-2 rounded-lg bg-muted/40 p-1">
              <button
                v-for="filter in quoteFilters"
                :key="filter.key"
                type="button"
                class="rounded-md px-3 py-1.5 text-xs font-bold transition-colors"
                :class="activeQuote === filter.key ? 'bg-background text-primary shadow-sm' : 'text-muted-foreground hover:text-foreground'"
                @click="activeQuote = filter.key"
              >
                {{ filter.label }}
              </button>
            </div>
          </div>

          <div class="flex flex-wrap items-center justify-between gap-3">
            <p class="text-sm text-muted-foreground">
              {{ t('market.showing_count', { count: sortedMarkets.length }) }}
            </p>
            <div class="flex flex-wrap gap-2">
              <button
                v-for="option in sortOptions"
                :key="option.key"
                type="button"
                class="rounded-lg border border-border px-3 py-1.5 text-xs font-bold transition-colors hover:border-primary hover:text-primary"
                :class="sortKey === option.key ? 'border-primary bg-primary/10 text-primary' : 'text-muted-foreground'"
                @click="sortKey = option.key"
              >
                {{ t(option.labelKey) }}
              </button>
            </div>
          </div>
        </div>

        <div class="overflow-x-auto">
          <table class="w-full min-w-[920px] text-left text-sm">
            <thead>
              <tr class="border-b border-border text-xs font-bold uppercase text-muted-foreground">
                <th class="w-12 px-4 py-3"></th>
                <th class="px-4 py-3">{{ t('market.name') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.price') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.change_24h') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.high_24h') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.low_24h') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.volume_24h') }}</th>
                <th class="px-4 py-3 text-right">{{ t('market.turnover_24h') }}</th>
                <th class="px-4 py-3 text-right">{{ t('trade.action') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="ticker in sortedMarkets"
                :key="ticker.symbol"
                class="border-b border-border/60 transition-colors hover:bg-muted/40"
              >
                <td class="px-4 py-4">
                  <button
                    type="button"
                    class="flex h-8 w-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-primary"
                    :aria-label="isFavorite(ticker.symbol) ? t('market.remove_favorite') : t('market.add_favorite')"
                    @click.stop="toggleFavorite(ticker.symbol)"
                  >
                    <Icon :icon="isFavorite(ticker.symbol) ? 'mdi:star' : 'mdi:star-outline'" class="h-5 w-5" :class="{ 'text-primary': isFavorite(ticker.symbol) }" />
                  </button>
                </td>
                <td class="px-4 py-4">
                  <button type="button" class="flex items-center gap-3 text-left" @click="goToTrade(ticker.symbol)">
                    <PairLogo class="h-9 w-9 shrink-0" :symbol="ticker.symbol" :src="ticker.icon" />
                    <span>
                      <span class="block font-bold">{{ ticker.symbol }}</span>
                      <span class="text-xs text-muted-foreground">{{ quoteAsset(ticker.symbol) }} {{ t('market.market') }}</span>
                    </span>
                  </button>
                </td>
                <td class="px-4 py-4 text-right font-mono font-bold">{{ formatNumber(ticker.close, 'price') }}</td>
                <td class="px-4 py-4 text-right">
                  <span class="rounded-md px-2 py-1 text-xs font-bold" :class="changeBadgeClass(ticker.chg)">
                    {{ formatSignedChange(ticker.chg) }}
                  </span>
                </td>
                <td class="px-4 py-4 text-right font-mono text-muted-foreground">{{ formatNumber(ticker.high, 'price') }}</td>
                <td class="px-4 py-4 text-right font-mono text-muted-foreground">{{ formatNumber(ticker.low, 'price') }}</td>
                <td class="px-4 py-4 text-right font-mono text-muted-foreground">{{ formatNumber(ticker.volume, 'volume') }}</td>
                <td class="px-4 py-4 text-right font-mono text-muted-foreground">{{ formatNumber(ticker.turnover, 'volume') }}</td>
                <td class="px-4 py-4 text-right">
                  <button
                    type="button"
                    class="rounded-lg bg-primary/10 px-3 py-1.5 text-xs font-bold text-primary transition-colors hover:bg-primary hover:text-primary-foreground"
                    @click="goToTrade(ticker.symbol)"
                  >
                    {{ t('nav.trade') }}
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div v-if="loading" class="flex items-center justify-center gap-2 px-4 py-12 text-sm text-muted-foreground">
          <Icon icon="mdi:loading" class="h-5 w-5 animate-spin text-primary" />
          {{ t('common.loading') }}
        </div>
        <div v-else-if="sortedMarkets.length === 0" class="px-4 py-16 text-center text-sm text-muted-foreground">
          {{ t('market.no_markets') }}
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Icon } from '@iconify/vue'
import numeral from 'numeral'
import { fetchMarketSnapshot } from '@/api/market'
import PairLogo from '@/components/common/PairLogo.vue'
import { useMarketStore, type Ticker } from '@/stores/market'
import { formatNumber } from '@/utils/format'

type MarketTab = 'favorites' | 'all' | 'spot' | 'futures'
type SortKey = 'popular' | 'gainers' | 'losers' | 'volume'

const FAVORITES_STORAGE_KEY = 'pc.market.favoriteSymbols'

const router = useRouter()
const marketStore = useMarketStore()
const { t } = useI18n()

const searchText = ref('')
const activeTab = ref<MarketTab>('all')
const activeQuote = ref('all')
const sortKey = ref<SortKey>('popular')
const loading = ref(false)
const favoriteSymbols = ref<string[]>(readFavorites())

const marketTabs: Array<{ key: MarketTab; labelKey: string }> = [
  { key: 'favorites', labelKey: 'market.favorites' },
  { key: 'all', labelKey: 'market.all_markets' },
  { key: 'spot', labelKey: 'market.spot_markets' },
  { key: 'futures', labelKey: 'market.futures_markets' },
]

const sortOptions: Array<{ key: SortKey; labelKey: string }> = [
  { key: 'popular', labelKey: 'market.sort_popular' },
  { key: 'gainers', labelKey: 'market.sort_gainers' },
  { key: 'losers', labelKey: 'market.sort_losers' },
  { key: 'volume', labelKey: 'market.sort_volume' },
]

const tickers = computed(() => marketStore.tickers)

const quoteFilters = computed(() => {
  const quoteSet = new Set(tickers.value.map(ticker => quoteAsset(ticker.symbol)).filter(Boolean))
  const quotes = Array.from(quoteSet).sort()
  return [
    { key: 'all', label: t('common.all') },
    ...quotes.slice(0, 5).map(quote => ({ key: quote, label: quote })),
  ]
})

const filteredMarkets = computed(() => {
  const keyword = searchText.value.trim().toLowerCase()
  return tickers.value.filter(ticker => {
    const symbol = ticker.symbol.toLowerCase()
    const quote = quoteAsset(ticker.symbol)
    if (keyword && !symbol.includes(keyword)) return false
    if (activeTab.value === 'favorites' && !favoriteSymbols.value.includes(ticker.symbol)) return false
    if (activeTab.value === 'futures') return false
    if (activeQuote.value !== 'all' && quote !== activeQuote.value) return false
    return true
  })
})

const sortedMarkets = computed(() => sortTickers(filteredMarkets.value, sortKey.value))

const overviewCards = computed(() => [
  {
    key: 'hot',
    title: t('market.hot_coins'),
    subtitle: t('market.hot_coins_desc'),
    icon: 'mdi:fire',
    items: topByTurnover.value,
  },
  {
    key: 'new',
    title: t('market.new_listings'),
    subtitle: t('market.new_listings_desc'),
    icon: 'mdi:sparkles',
    items: newestMarkets.value,
  },
  {
    key: 'gainers',
    title: t('market.top_gainers'),
    subtitle: t('market.top_gainers_desc'),
    icon: 'mdi:trending-up',
    items: topGainers.value,
  },
  {
    key: 'volume',
    title: t('market.top_volume'),
    subtitle: t('market.top_volume_desc'),
    icon: 'mdi:chart-bar',
    items: topByVolume.value,
  },
])

const topGainers = computed(() => sortTickers(tickers.value, 'gainers').slice(0, 3))
const topByVolume = computed(() => sortTickers(tickers.value, 'volume').slice(0, 3))
const topByTurnover = computed(() => [...tickers.value].sort((a, b) => (b.turnover || 0) - (a.turnover || 0)).slice(0, 3))
const newestMarkets = computed(() => [...tickers.value].sort((a, b) => (b.time || 0) - (a.time || 0)).slice(0, 3))

watch(favoriteSymbols, (value) => {
  window.localStorage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(value))
}, { deep: true })

function readFavorites() {
  try {
    const raw = window.localStorage.getItem(FAVORITES_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : []
  } catch {
    return []
  }
}

function sortTickers(list: Ticker[], key: SortKey) {
  const sorted = [...list]
  switch (key) {
    case 'gainers':
      return sorted.sort((a, b) => (b.chg || 0) - (a.chg || 0))
    case 'losers':
      return sorted.sort((a, b) => (a.chg || 0) - (b.chg || 0))
    case 'volume':
      return sorted.sort((a, b) => (b.volume || 0) - (a.volume || 0))
    default:
      return sorted.sort((a, b) => (b.turnover || 0) - (a.turnover || 0))
  }
}

function quoteAsset(symbol: string) {
  return symbol.split('/')[1] || ''
}

function changeClass(change: number) {
  return change >= 0 ? 'text-up' : 'text-down'
}

function changeBadgeClass(change: number) {
  return change >= 0 ? 'bg-up/10 text-up' : 'bg-down/10 text-down'
}

function formatSignedChange(change: number) {
  return `${change >= 0 ? '+' : ''}${numeral(change || 0).format('0.00')}%`
}

function isFavorite(symbol: string) {
  return favoriteSymbols.value.includes(symbol)
}

function toggleFavorite(symbol: string) {
  favoriteSymbols.value = isFavorite(symbol)
    ? favoriteSymbols.value.filter(item => item !== symbol)
    : [...favoriteSymbols.value, symbol]
}

function goToTrade(symbol: string) {
  marketStore.setActiveSymbol(symbol)
  router.push({ name: 'Trade', params: { symbol: symbol.replace('/', '_') } })
}

async function loadMarkets() {
  loading.value = true
  try {
    const response = await fetchMarketSnapshot()
    const res = response.data
    if (Array.isArray(res)) {
      marketStore.setTickers(res)
    } else if (res && Array.isArray(res.data)) {
      marketStore.setTickers(res.data)
    }
  } catch (error) {
    console.error('Failed to fetch market data:', error)
  } finally {
    loading.value = false
  }
}

onMounted(loadMarkets)
</script>
