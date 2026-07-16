<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { RefreshCw, Search } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { fallbackTickers } from '@/data/fallback'
import { formatCompact, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const navigation = useNavigationStore()
const { t } = useI18n()
const query = ref('')
type MarketCategory = 'popular' | 'gainers' | 'losers'
const category = ref<MarketCategory>('popular')
const categories = computed(() => [
  { key: 'popular' as const, label: t('markets.popular') },
  { key: 'gainers' as const, label: t('markets.gainers') },
  { key: 'losers' as const, label: t('markets.losers') },
])
const pickerMode = computed(() => route.query.purpose === 'trade')
const title = computed(() => pickerMode.value ? t('markets.pickerTitle') : t('markets.title'))

const rows = computed(() => {
  const source = marketStore.tickers.length ? [...marketStore.tickers] : [...fallbackTickers]
  const keyword = query.value.trim().toUpperCase()
  const filtered = keyword ? source.filter((item) => item.symbol.includes(keyword)) : source
  if (category.value === 'gainers') return filtered.sort((left, right) => right.changePercent - left.changePercent)
  if (category.value === 'losers') return filtered.sort((left, right) => left.changePercent - right.changePercent)
  return filtered.sort((left, right) => right.volume - left.volume)
})

const turnover = computed(() => rows.value.reduce((total, item) => total + item.volume * item.lastPrice, 0))
const positiveRate = computed(() => rows.value.length ? (rows.value.filter((item) => item.changePercent >= 0).length / rows.value.length) * 100 : 0)

function openMarket(symbol: string) {
  const routeSymbol = symbol.replace('/', '_')
  if (pickerMode.value) {
    navigation.rememberTradeSymbol(routeSymbol)
    const mode = route.query.mode === 'contract' ? 'contract' : 'spot'
    navigation.rememberTradeMode(mode)
    void router.replace({ name: 'trade', params: { symbol: routeSymbol }, query: mode === 'contract' ? { mode } : undefined })
    return
  }
  void router.push({ name: 'market-detail', params: { symbol: routeSymbol } })
}

function selectCategory(next: MarketCategory) {
  category.value = next
}

onMounted(() => { void marketStore.refresh() })
</script>

<template>
  <main class="page markets-page">
    <PageHeader :title="title" :back="pickerMode" :fallback="navigation.lastTradePath">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('markets.refresh')" :disabled="marketStore.loading" @click="marketStore.refresh(true)"><RefreshCw :size="21" :class="{ spin: marketStore.loading }" /></button>
      </template>
    </PageHeader>
    <div class="page-content">
      <label class="market-search"><Search :size="20" /><input v-model="query" type="search" :placeholder="t('markets.searchPlaceholder')" /></label>
      <section class="market-overview" :aria-label="t('markets.overview')">
        <div><span>{{ t('markets.turnover24h') }}</span><strong><b>{{ formatCompact(turnover) }}</b><small>USD</small></strong></div>
        <div><span>{{ t('markets.advancingShare') }}</span><strong class="up"><b>{{ positiveRate.toFixed(1) }}%</b></strong></div>
        <div><span>{{ t('markets.marketCount') }}</span><strong><b>{{ rows.length }}</b></strong></div>
      </section>

      <div class="market-category" role="tablist">
        <button v-for="item in categories" :key="item.key" type="button" :class="{ 'is-active': category === item.key }" @click="selectCategory(item.key)">{{ item.label }}</button>
      </div>
      <p v-if="marketStore.sampleData" class="sample-note">{{ t('common.offlineMarketNotice') }}</p>

      <div class="market-list__heading"><span>{{ t('markets.pair') }}</span><span>{{ t('markets.latestPrice') }}</span><span>{{ t('markets.change24h') }}</span></div>
      <div class="market-list">
        <button v-for="ticker in rows" :key="ticker.symbol" class="market-list__row" type="button" @click="openMarket(ticker.symbol)">
          <span class="market-list__symbol"><AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :size="36" /><span><b>{{ ticker.base }}/{{ ticker.quote }}</b><small>{{ t('markets.volume', { value: formatCompact(ticker.volume) }) }}</small></span></span>
          <span class="market-list__price"><b>{{ formatPrice(ticker.lastPrice) }}</b><small>≈ {{ formatPrice(ticker.lastPrice) }} USD</small></span>
          <span class="market-list__change" :class="ticker.changePercent >= 0 ? 'up' : 'down'">{{ formatPercent(ticker.changePercent) }}</span>
        </button>
      </div>
      <p v-if="!rows.length" class="empty-state">{{ t('markets.noResults') }}</p>
    </div>
  </main>
</template>

<style scoped>
.market-search { align-items: center; background: var(--soft); border-radius: 26px; color: var(--muted); display: flex; gap: 10px; min-height: 50px; padding: 0 16px; }.market-search input { background: transparent; border: 0; color: var(--ink); min-width: 0; outline: 0; width: 100%; }.market-search input::placeholder { color: #9ea3a9; }
.market-overview { display: grid; gap: 8px; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 24px 0; }.market-overview div { background: var(--soft); border-radius: var(--radius); display: grid; gap: 8px; min-height: 88px; padding: 12px 10px; }.market-overview span { color: var(--muted); font-size: 11px; line-height: 1.3; }.market-overview strong { align-items: baseline; display: flex; flex-wrap: wrap; font-size: 14px; gap: 3px; min-width: 0; }.market-overview strong b { font: inherit; white-space: nowrap; }.market-overview strong small { color: var(--muted); font-size: 10px; font-weight: 650; }
.market-category { border-bottom: 1px solid var(--line); display: flex; gap: 28px; }.market-category button { background: transparent; border-bottom: 2px solid transparent; color: var(--muted); font-size: 16px; min-height: 42px; padding: 0; }.market-category .is-active { border-color: var(--ink); color: var(--ink); font-weight: 750; }.sample-note { background: #fff8e6; border-radius: 6px; color: #8a5a00; font-size: 12px; margin: 12px 0 5px; padding: 7px 9px; }
.market-list__heading { color: var(--muted); display: grid; font-size: 11px; grid-template-columns: minmax(0, 1.2fr) .9fr .65fr; padding: 17px 0 5px; }.market-list__heading span:nth-child(n+2) { text-align: right; }
.market-list__row { align-items: center; background: transparent; display: grid; grid-template-columns: minmax(0, 1.2fr) .9fr .65fr; min-height: 72px; padding: 8px 0; text-align: left; width: 100%; }.market-list__symbol { align-items: center; display: flex; gap: 10px; min-width: 0; }.market-list__symbol span,.market-list__price { display: grid; min-width: 0; }.market-list__row b { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.market-list__row small { color: var(--muted); font-size: 11px; margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.market-list__price { text-align: right; }.market-list__change { font-size: 13px; font-weight: 700; text-align: right; }
.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
