<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CandlestickChart, Gauge, Layers3, RefreshCw, Search } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { formatCompact, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const navigation = useNavigationStore()
const { t } = useI18n()

const query = ref('')
type MarketCategory = 'popular' | 'favorites' | 'spot' | 'contract' | 'gainers'
type TradeMode = 'spot' | 'contract'
const category = ref<MarketCategory>('popular')

const categories = computed(() => [
  { key: 'popular' as const, label: t('markets.popular') },
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'spot' as const, label: t('trade.spot') },
  { key: 'contract' as const, label: t('trade.contract') },
  { key: 'gainers' as const, label: t('markets.gainers') },
])

const pickerMode = computed(() => route.query.purpose === 'trade')
const title = computed(() => pickerMode.value ? t('markets.pickerTitle') : t('markets.title'))

const rows = computed(() => {
  const source = [...marketStore.tickers]
  const keyword = query.value.trim().toUpperCase()
  const filtered = keyword ? source.filter((item) => item.symbol.includes(keyword)) : source
  if (category.value === 'gainers') return filtered.sort((left, right) => right.changePercent - left.changePercent)
  if (category.value === 'popular') return filtered.sort((left, right) => right.volume - left.volume)
  return filtered
})

const turnover = computed(() => rows.value.reduce((total, item) => total + item.volume * item.lastPrice, 0))
const positiveRate = computed(() => rows.value.length
  ? (rows.value.filter((item) => item.changePercent >= 0).length / rows.value.length) * 100
  : 0)
const marketTemperature = computed(() => Math.round(positiveRate.value))

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

function openTradeMode(mode: TradeMode): void {
  navigation.rememberTradeMode(mode)
  void router.replace({
    name: 'trade',
    params: { symbol: navigation.lastTradeSymbol },
    query: mode === 'contract' ? { mode } : undefined,
  })
}

function selectCategory(next: MarketCategory): void {
  category.value = next
}

onMounted(async () => {
  await marketStore.refresh()
  marketStore.startLiveUpdates()
})
onUnmounted(() => marketStore.stopLiveUpdates())
</script>

<template>
  <main class="page markets-page">
    <PageHeader :title="title" :eyebrow="t('markets.overview')" :back="pickerMode" :fallback="navigation.lastTradePath">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('markets.refresh')" :disabled="marketStore.loading" @click="marketStore.refresh(true)">
          <RefreshCw :size="20" :class="{ spin: marketStore.loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content">
      <section class="market-intro" :aria-label="t('markets.overview')">
        <span>{{ t('markets.overview') }}</span>
        <h1>{{ title }}</h1>
        <div class="market-intro__metrics">
          <span>
            <small>{{ t('markets.turnover24h') }}</small>
            <strong class="numeric">{{ formatCompact(turnover) }}</strong>
          </span>
          <span>
            <small>{{ t('markets.marketCount') }}</small>
            <strong class="numeric">{{ rows.length }}</strong>
          </span>
        </div>
      </section>

      <nav v-if="!pickerMode" class="market-destinations" :aria-label="t('trade.category')">
        <button data-market-destination="spot" type="button" @click="openTradeMode('spot')"><CandlestickChart :size="19" /><span>{{ t('trade.spot') }}</span></button>
        <button data-market-destination="seconds" type="button" @click="router.replace({ name: 'seconds' })"><Gauge :size="19" /><span>{{ t('seconds.title') }}</span></button>
        <button data-market-destination="contract" type="button" @click="openTradeMode('contract')"><Layers3 :size="19" /><span>{{ t('trade.contract') }}</span></button>
      </nav>

      <label class="market-search">
        <Search :size="19" />
        <input v-model="query" type="search" :aria-label="t('markets.searchPlaceholder')" :placeholder="t('markets.searchPlaceholder')" />
      </label>

      <div class="market-category" role="tablist" :aria-label="t('markets.overview')">
        <button
          v-for="item in categories"
          :key="item.key"
          type="button"
          role="tab"
          :aria-selected="category === item.key"
          :class="{ 'is-active': category === item.key }"
          @click="selectCategory(item.key)"
        >
          {{ item.label }}
        </button>
      </div>

      <section class="market-temperature" :aria-label="t('markets.advancingShare')">
        <div>
          <span>{{ t('markets.advancingShare') }}</span>
          <strong class="numeric">{{ marketTemperature }}</strong>
        </div>
        <span class="market-temperature__track" aria-hidden="true"><i :style="{ width: `${marketTemperature}%` }" /></span>
        <small class="numeric">{{ positiveRate.toFixed(1) }}%</small>
      </section>

      <div v-if="marketStore.error" class="market-error">
        <span>{{ t('common.marketLoadFailed') }}</span>
        <button type="button" :disabled="marketStore.loading" @click="marketStore.refresh(true)">{{ t('common.retry') }}</button>
      </div>

      <div class="market-list__heading">
        <span>{{ t('markets.pair') }}</span>
        <span>{{ t('markets.latestPrice') }}</span>
        <span>{{ t('markets.change24h') }}</span>
      </div>
      <div class="market-list">
        <button v-for="ticker in rows" :key="ticker.symbol" class="market-list__row" type="button" @click="openMarket(ticker.symbol)">
          <span class="market-list__symbol">
            <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :size="32" />
            <span><b>{{ ticker.base }}<small>/{{ ticker.quote }}</small></b><em>{{ t('markets.volume', { value: formatCompact(ticker.volume) }) }}</em></span>
          </span>
          <span class="market-list__spark" :class="ticker.changePercent >= 0 ? 'is-up' : 'is-down'" aria-hidden="true" />
          <span class="market-list__price"><b>{{ formatPrice(ticker.lastPrice) }}</b><small>{{ ticker.quote }}</small></span>
          <span class="market-list__change" :class="ticker.changePercent >= 0 ? 'is-up' : 'is-down'">{{ formatPercent(ticker.changePercent) }}</span>
        </button>
      </div>
      <p v-if="!rows.length && !marketStore.error" class="empty-state">{{ t('markets.noResults') }}</p>
    </div>
  </main>
</template>

<style scoped>
.markets-page .page-content {
  padding-top: 14px;
}

.markets-page {
  --markets-contrast-ink: var(--surface);
}

:global(:root[data-theme='dark']) .markets-page {
  --markets-contrast-ink: var(--ink);
}

.market-signal {
  background: var(--signal-green);
  border-bottom: 1px solid var(--line-strong);
  color: var(--on-positive);
  display: grid;
  grid-template-columns: 1.25fr 1fr .72fr;
  margin: 0 -16px;
  min-height: 124px;
  overflow: hidden;
  padding: 20px 16px;
  position: relative;
}

.market-signal::after {
  bottom: -24px;
  color: color-mix(in srgb, var(--on-positive) 18%, transparent);
  content: '///';
  font-family: var(--data-font);
  font-size: 84px;
  line-height: 1;
  pointer-events: none;
  position: absolute;
  right: 10px;
}

.market-signal > div {
  align-content: center;
  display: grid;
  gap: 8px;
  min-width: 0;
  position: relative;
  z-index: 1;
}

.market-signal > div + div {
  border-left: 1px solid color-mix(in srgb, var(--on-positive) 22%, transparent);
  padding-left: 13px;
}

.market-signal span {
  color: color-mix(in srgb, var(--on-positive) 72%, transparent);
  font-size: 10px;
  line-height: 1.25;
}

.market-signal strong {
  color: var(--on-positive);
  font-family: var(--data-font);
  font-size: 17px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-signal strong.up {
  color: var(--on-positive);
}

.market-signal small {
  color: inherit;
  font-size: 9px;
  font-weight: 650;
}

.market-destinations {
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0 -16px;
}

.market-destinations button {
  align-items: center;
  background: var(--surface);
  color: var(--ink);
  display: flex;
  font-size: 11px;
  font-weight: 700;
  gap: 7px;
  justify-content: center;
  min-height: 54px;
  min-width: 0;
  padding: 0 6px;
}

.market-destinations button + button {
  border-left: 1px solid var(--line);
}

.market-destinations button:nth-child(1) svg {
  color: var(--positive);
}

.market-destinations button:nth-child(2) {
  background: var(--soft);
}

.market-destinations button:nth-child(2) svg {
  color: var(--accent);
}

.market-destinations button:nth-child(3) svg {
  color: var(--negative);
}

.market-destinations span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-search {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: 0;
  color: var(--muted);
  display: flex;
  gap: 9px;
  margin-top: 14px;
  min-height: 48px;
  padding: 0 14px;
}

.market-search:focus-within {
  border-color: var(--focus, var(--accent));
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.market-search input {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-width: 0;
  outline: 0;
  width: 100%;
}

.market-search input::placeholder {
  color: var(--muted);
}

.market-category {
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  margin-top: 10px;
}

.market-category button {
  background: transparent;
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-size: 12px;
  min-height: 44px;
  padding: 0 6px;
}

.market-category .is-active {
  border-color: var(--signal-green);
  color: var(--ink);
  font-weight: 750;
}

.market-error {
  align-items: center;
  background: var(--negative-soft);
  border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
  display: flex;
  font-size: 12px;
  gap: 10px;
  justify-content: space-between;
  margin: 12px 0 4px;
  padding: 8px 10px;
}

.market-error button {
  background: transparent;
  color: var(--negative);
  font-weight: 750;
  min-height: 44px;
  padding: 0 8px;
}

.market-list__heading {
  color: var(--muted);
  display: grid;
  font-size: 10px;
  grid-template-columns: minmax(0, 1.16fr) minmax(76px, .8fr) 76px;
  min-height: 39px;
  padding-top: 7px;
}

.market-list__heading span {
  align-self: center;
}

.market-list__heading span:nth-child(n + 2) {
  text-align: right;
}

.market-list__row {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1.16fr) minmax(76px, .8fr) 76px;
  min-height: 70px;
  padding: 7px 0;
  text-align: left;
  width: 100%;
}

.market-list__symbol {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.market-list__symbol > span,
.market-list__price {
  display: grid;
  min-width: 0;
}

.market-list__row b {
  color: var(--ink);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-list__symbol b small {
  display: inline;
  font-size: 10px;
  font-weight: 600;
  margin: 0;
}

.market-list__row small,
.market-list__row em {
  color: var(--muted);
  font-size: 10px;
  font-style: normal;
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-list__price {
  text-align: right;
}

.market-list__change {
  border: 1px solid currentColor;
  font-size: 11px;
  font-weight: 730;
  min-height: 32px;
  padding: 8px 4px;
  text-align: center;
}

.market-list__change.is-up {
  background: var(--positive);
  border-color: var(--positive);
  color: var(--on-positive);
}

.market-list__change.is-down {
  background: var(--negative);
  border-color: var(--negative);
  color: var(--on-negative);
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 360px) {
  .market-signal,
  .market-destinations {
    margin-left: -12px;
    margin-right: -12px;
  }

  .market-signal {
    padding-left: 12px;
    padding-right: 12px;
  }
}

@media (max-width: 340px) {
  .markets-page .page-content {
    padding-left: 12px;
    padding-right: 12px;
  }

  .market-signal,
  .market-destinations {
    margin-left: -12px;
    margin-right: -12px;
  }

  .market-signal {
    padding-left: 12px;
    padding-right: 12px;
  }

  .market-list__heading,
  .market-list__row {
    grid-template-columns: minmax(0, 1fr) minmax(70px, .72fr) 70px;
  }

  .market-list__symbol {
    gap: 6px;
  }

  .market-list__symbol em {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>

<style scoped>
.markets-page {
  background-color: var(--surface);
  background-image:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px);
  background-size: 48px 48px;
}

.markets-page .page-content {
  padding-top: 0;
}

.market-intro {
  background: var(--signal-green);
  border-bottom: 1px solid var(--line-strong);
  color: var(--on-positive);
  margin: 0 -16px;
  min-height: 176px;
  overflow: hidden;
  padding: 25px 20px 18px;
  position: relative;
}

.market-intro::after {
  bottom: -28px;
  color: color-mix(in srgb, var(--on-positive) 16%, transparent);
  content: '///';
  font-family: var(--data-font);
  font-size: 100px;
  line-height: 1;
  position: absolute;
  right: 14px;
}

.market-intro > span {
  font-family: var(--data-font);
  font-size: 10px;
  font-weight: 700;
}

.market-intro h1 {
  font-size: 38px;
  line-height: 1;
  margin: 14px 0 19px;
  position: relative;
  z-index: 1;
}

.market-intro__metrics {
  display: flex;
  gap: 26px;
  position: relative;
  z-index: 1;
}

.market-intro__metrics span {
  display: grid;
  gap: 3px;
}

.market-intro__metrics small {
  color: color-mix(in srgb, var(--on-positive) 68%, transparent);
  font-size: 9px;
}

.market-intro__metrics strong {
  font-size: 13px;
}

.market-destinations {
  margin: 0 -16px;
}

.market-search {
  background: var(--surface);
  margin-top: 14px;
}

.market-category {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  margin-top: 10px;
}

.market-category button {
  font-size: 10px;
  min-width: 0;
  overflow: hidden;
  padding: 0 2px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-category .is-active {
  background: var(--signal-green);
  border-bottom-color: var(--signal-green);
  color: var(--on-positive);
}

.market-temperature {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  display: grid;
  gap: 12px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  margin: 14px 0 18px;
  min-height: 70px;
  padding: 12px 14px;
}

.market-temperature > div {
  align-items: baseline;
  display: flex;
  gap: 7px;
}

.market-temperature span,
.market-temperature small {
  color: var(--muted);
  font-size: 9px;
}

.market-temperature strong {
  color: var(--ink);
  font-size: 22px;
}

.market-temperature__track {
  background: var(--soft);
  display: block;
  height: 3px;
  position: relative;
}

.market-temperature__track i {
  background: var(--positive);
  display: block;
  height: 100%;
  max-width: 100%;
  min-width: 0;
  position: relative;
}

.market-temperature__track i::after {
  background: var(--positive);
  border-radius: 50%;
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--positive) 14%, transparent);
  content: '';
  height: 8px;
  position: absolute;
  right: 0;
  top: 50%;
  transform: translate(50%, -50%);
  width: 8px;
}

.market-list__heading,
.market-list__row {
  grid-template-columns: minmax(0, 1fr) 62px minmax(72px, .82fr) 70px;
}

.market-list__heading span:nth-child(2) {
  grid-column: 3;
}

.market-list__heading span:nth-child(3) {
  grid-column: 4;
}

.market-list__spark {
  color: var(--positive);
  height: 34px;
  overflow: hidden;
  position: relative;
  width: 62px;
}

.market-list__spark::before,
.market-list__spark::after {
  background: currentColor;
  content: '';
  height: 2px;
  left: 4px;
  position: absolute;
  top: 18px;
  transform: rotate(-9deg);
  transform-origin: left center;
  width: 56px;
}

.market-list__spark::after {
  left: 30px;
  top: 14px;
  transform: rotate(11deg);
  width: 28px;
}

.market-list__spark.is-down {
  color: var(--negative);
  transform: scaleY(-1);
}

@media (max-width: 360px) {
  .market-intro,
  .market-destinations {
    margin-left: -12px;
    margin-right: -12px;
  }

  .market-intro {
    padding-left: 12px;
    padding-right: 12px;
  }

  .market-list__heading,
  .market-list__row {
    grid-template-columns: minmax(0, 1fr) 52px 68px 64px;
    gap: 6px;
  }

  .market-list__spark {
    width: 52px;
  }

  .market-list__spark::before {
    width: 46px;
  }
}

@media (max-width: 340px) {
  .market-intro h1 {
    font-size: 32px;
  }

  .market-list__symbol em {
    display: none;
  }
}
</style>
