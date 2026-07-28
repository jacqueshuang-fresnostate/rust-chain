<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ArrowLeftRight,
  ChartNoAxesCombined,
  CircleAlert,
  RefreshCw,
  Share2,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import MobileMarketChart from '@/components/MobileMarketChart.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { fetchKlines, fetchOrderBook, fetchRecentTrades } from '@/api/market'
import { formatAmount, formatPercent, formatPrice } from '@/core/format'
import { currentIntlLocale } from '@/i18n'
import { goBackOr } from '@/core/navigation'
import { useMarketStore } from '@/stores/market'
import type { KlinePoint, OrderBookLevel, TradePrint } from '@/core/types'

const props = defineProps<{ symbol: string }>()
const router = useRouter()
const marketStore = useMarketStore()
const { t } = useI18n()
const interval = ref('15m')
const loading = ref(true)
const dataError = ref(false)
const points = ref<KlinePoint[]>([])
const bids = ref<OrderBookLevel[]>([])
const asks = ref<OrderBookLevel[]>([])
const trades = ref<TradePrint[]>([])
let requestVersion = 0

const pairSymbol = computed(() => props.symbol.replace(/[_-]/g, '/').toUpperCase())
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const latestPrice = computed(() => ticker.value?.lastPrice ?? 0)

async function load(forceMarket = false): Promise<void> {
  const version = ++requestVersion
  loading.value = true
  dataError.value = false
  points.value = []
  bids.value = []
  asks.value = []
  trades.value = []
  void marketStore.refresh(forceMarket)
  const [klineResult, depthResult, tradesResult] = await Promise.allSettled([
    fetchKlines(pairSymbol.value, interval.value),
    fetchOrderBook(pairSymbol.value),
    fetchRecentTrades(pairSymbol.value),
  ])
  if (version !== requestVersion) return

  const hasKlines = klineResult.status === 'fulfilled' && klineResult.value.length > 0
  const hasDepth = depthResult.status === 'fulfilled' && (depthResult.value.bids.length > 0 || depthResult.value.asks.length > 0)
  const hasTrades = tradesResult.status === 'fulfilled' && tradesResult.value.length > 0
  dataError.value = [klineResult, depthResult, tradesResult].some((result) => result.status === 'rejected')
  points.value = hasKlines ? klineResult.value : []
  bids.value = hasDepth ? depthResult.value.bids : []
  asks.value = hasDepth ? depthResult.value.asks : []
  trades.value = hasTrades ? tradesResult.value : []
  loading.value = false
}

function retry(): void {
  void load(true)
}

function chooseInterval(value: string): void {
  if (interval.value === value) return
  interval.value = value
  void load()
}

function openTrade(mode: 'spot' | 'contract' = 'spot'): void {
  void router.replace({
    name: 'trade',
    params: { symbol: pairSymbol.value.replace('/', '_') },
    query: mode === 'contract' ? { mode: 'contract' } : undefined,
  })
}

async function shareMarket(): Promise<void> {
  const url = window.location.href
  try {
    if (navigator.share) {
      await navigator.share({ title: pairSymbol.value, url })
      return
    }
    await navigator.clipboard?.writeText(url)
  } catch {
    return
  }
}

function goBack(): void {
  void goBackOr(router, { name: 'markets' })
}

watch(() => props.symbol, () => { void load() }, { immediate: true })

onUnmounted(() => {
  requestVersion += 1
})
</script>

<template>
  <main class="market-detail">
    <header class="market-detail__header">
      <button class="icon-button" type="button" :aria-label="t('common.back')" @click="goBack">
        <ArrowLeft :size="24" />
      </button>
      <div class="market-detail__instrument">
        <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :size="34" />
        <span>
          <strong>{{ baseAsset }}/{{ quoteAsset }}</strong>
          <small>{{ t('marketDetail.spot') }}</small>
        </span>
      </div>
      <button class="icon-button" type="button" :aria-label="t('marketDetail.share')" @click="shareMarket">
        <Share2 :size="20" />
      </button>
    </header>

    <nav class="market-detail__tabs" :aria-label="t('marketDetail.details')">
      <button class="is-active" type="button" aria-current="page">
        {{ t('marketDetail.market') }}
      </button>
      <button type="button" @click="openTrade('spot')">
        {{ t('marketDetail.trade') }}
      </button>
    </nav>

    <section class="market-detail__price" :aria-busy="marketStore.loading">
      <div class="market-detail__quote">
        <span>{{ t('marketDetail.latestPrice') }}</span>
        <strong v-if="ticker" class="numeric" :class="ticker.changePercent >= 0 ? 'up' : 'down'">
          {{ formatPrice(latestPrice) }}
        </strong>
        <strong v-else class="numeric">--</strong>
        <p v-if="ticker">
          <span>{{ t('common.liveData') }}</span>
          <b :class="ticker.changePercent >= 0 ? 'up' : 'down'">
            {{ formatPercent(ticker.changePercent) }}
          </b>
        </p>
        <p v-else>{{ marketStore.loading ? t('common.loading') : t('common.marketUnavailable') }}</p>
      </div>
      <dl>
        <div>
          <dt>{{ t('marketDetail.high24h') }}</dt>
          <dd class="numeric">{{ ticker ? formatPrice(ticker.highPrice) : '--' }}</dd>
        </div>
        <div>
          <dt>{{ t('marketDetail.low24h') }}</dt>
          <dd class="numeric">{{ ticker ? formatPrice(ticker.lowPrice) : '--' }}</dd>
        </div>
        <div>
          <dt>{{ t('marketDetail.volume24h') }}</dt>
          <dd class="numeric">{{ ticker ? `${formatAmount(ticker.volume)} ${baseAsset}` : '--' }}</dd>
        </div>
      </dl>
    </section>

    <div v-if="dataError || marketStore.error" class="market-detail__error" role="alert">
      <CircleAlert :size="18" />
      <span>{{ t('common.marketLoadFailed') }}</span>
      <button type="button" :disabled="loading" :aria-label="t('common.retry')" @click="retry">
        <RefreshCw :size="17" :class="{ spin: loading }" />
      </button>
    </div>

    <section class="market-detail__chart-panel">
      <header>
        <span>
          <ChartNoAxesCombined :size="18" />
          <strong>{{ t('marketDetail.market') }}</strong>
        </span>
        <small>{{ t('marketDetail.indicators') }}</small>
      </header>
      <nav class="market-detail__intervals" :aria-label="t('marketDetail.indicators')">
        <button
          v-for="item in ['1m', '15m', '1h', '4h', '1d']"
          :key="item"
          type="button"
          :class="{ 'is-active': interval === item }"
          :aria-pressed="interval === item"
          @click="chooseInterval(item)"
        >
          {{ item }}
        </button>
      </nav>
      <div class="market-detail__chart">
        <MobileMarketChart :points="points" :loading="loading" />
      </div>
    </section>

    <section class="market-detail__book">
      <OrderBookPanel
        :bids="bids"
        :asks="asks"
        :current-price="latestPrice"
        :loading="loading"
      />
    </section>

    <section class="market-detail__trades" :aria-busy="loading">
      <header><strong>{{ t('marketDetail.latestTrades') }}</strong></header>
      <div class="market-detail__trade-head">
        <span>{{ t('marketDetail.price') }}</span>
        <span>{{ t('marketDetail.quantity') }}</span>
        <span>{{ t('common.time') }}</span>
      </div>
      <div v-if="loading && !trades.length" class="market-detail__trade-state">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="!trades.length" class="market-detail__trade-state">
        {{ t('common.marketUnavailable') }}
      </div>
      <div v-else>
        <div v-for="trade in trades.slice(0, 8)" :key="trade.id" class="market-detail__trade">
          <span class="numeric" :class="trade.side === 'buy' ? 'up' : 'down'">
            {{ formatPrice(trade.price) }}
          </span>
          <span class="numeric">{{ formatAmount(trade.quantity) }}</span>
          <span class="numeric">
            {{ new Date(trade.time).toLocaleTimeString(currentIntlLocale(), { hour: '2-digit', minute: '2-digit' }) }}
          </span>
        </div>
      </div>
    </section>

    <nav class="market-detail__actions" :aria-label="t('marketDetail.actions')">
      <button class="is-primary" type="button" @click="openTrade('spot')">
        <ArrowLeftRight :size="20" />
        <span>{{ t('marketDetail.trade') }}</span>
      </button>
      <button type="button" @click="openTrade('contract')">
        <ChartNoAxesCombined :size="20" />
        <span>{{ t('marketDetail.contract') }}</span>
      </button>
    </nav>
  </main>
</template>

<style scoped>
.market-detail {
  background: var(--surface);
  color: var(--ink);
  min-height: 100dvh;
  min-width: 0;
  padding: env(safe-area-inset-top) 0 calc(74px + env(safe-area-inset-bottom));
}

.market-detail__header {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 44px;
  min-height: 60px;
  padding: 0 10px;
  position: sticky;
  top: env(safe-area-inset-top);
  z-index: var(--layer-sticky-header);
}

.market-detail__instrument {
  align-items: center;
  display: flex;
  gap: 9px;
  justify-content: center;
  min-width: 0;
}

.market-detail__instrument > span {
  display: grid;
  min-width: 0;
}

.market-detail__instrument strong {
  font-size: 17px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__instrument small {
  color: var(--muted);
  font-size: 10px;
  margin-top: 2px;
}

.market-detail__tabs {
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.market-detail__tabs button {
  background: transparent;
  border-bottom: 3px solid transparent;
  color: var(--muted);
  font-size: 13px;
  font-weight: 700;
  min-height: 48px;
  min-width: 0;
  padding: 0 8px;
}

.market-detail__tabs .is-active {
  border-color: var(--positive);
  color: var(--ink);
}

.market-detail__price {
  background:
    linear-gradient(110deg, color-mix(in srgb, var(--positive) 10%, transparent), transparent 56%),
    var(--surface-elevated);
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--positive);
  display: grid;
  gap: 16px;
  grid-template-columns: minmax(0, 1.15fr) minmax(124px, .85fr);
  min-width: 0;
  padding: 18px 20px 15px;
}

.market-detail__quote {
  min-width: 0;
}

.market-detail__quote > span {
  color: var(--muted);
  font-size: 11px;
}

.market-detail__quote > strong {
  display: block;
  font-size: 34px;
  letter-spacing: 0;
  line-height: 1.08;
  margin-top: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.market-detail__quote p {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  gap: 7px;
  margin: 8px 0 0;
}

.market-detail__quote p b {
  font-variant-numeric: tabular-nums;
}

.market-detail__price dl {
  align-self: center;
  display: grid;
  gap: 7px;
  margin: 0;
  min-width: 0;
}

.market-detail__price dl div {
  display: grid;
  font-size: 10px;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-width: 0;
}

.market-detail__price dt {
  color: var(--muted);
}

.market-detail__price dd {
  color: var(--muted-strong);
  margin: 0;
  max-width: 96px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__error {
  align-items: center;
  background: var(--negative-soft);
  border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: auto 1fr 44px;
  margin: 12px 20px 0;
  min-height: 48px;
  padding: 0 0 0 10px;
}

.market-detail__error button {
  align-items: center;
  background: transparent;
  color: var(--negative);
  display: inline-flex;
  justify-content: center;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
}

.market-detail__chart-panel {
  border-bottom: 1px solid var(--line);
  margin-top: 12px;
}

.market-detail__chart-panel > header {
  align-items: center;
  display: flex;
  justify-content: space-between;
  min-height: 44px;
  padding: 0 20px;
}

.market-detail__chart-panel > header span {
  align-items: center;
  color: var(--ink);
  display: inline-flex;
  font-size: 14px;
  gap: 7px;
}

.market-detail__chart-panel > header svg {
  color: var(--positive);
}

.market-detail__chart-panel > header small {
  color: var(--muted);
  font-size: 10px;
}

.market-detail__intervals {
  border-block: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.market-detail__intervals button {
  background: transparent;
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-size: 11px;
  min-height: 44px;
  min-width: 0;
  padding: 0 4px;
}

.market-detail__intervals .is-active {
  background: var(--positive-soft);
  border-color: var(--positive);
  color: var(--positive);
  font-weight: 800;
}

.market-detail__chart {
  height: 300px;
  position: relative;
}

.market-detail__book {
  border-bottom: 1px solid var(--line);
  border-top: 8px solid var(--soft);
}

.market-detail__book :deep(.order-book) {
  padding: 12px 20px 16px;
}

.market-detail__trades {
  border-top: 8px solid var(--soft);
  padding-bottom: 16px;
}

.market-detail__trades > header {
  align-items: center;
  display: flex;
  min-height: 48px;
  padding: 0 20px;
}

.market-detail__trades > header strong {
  font-size: 14px;
}

.market-detail__trade-head,
.market-detail__trade {
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) minmax(74px, .8fr) minmax(58px, .6fr);
  min-width: 0;
  padding-left: 20px;
  padding-right: 20px;
}

.market-detail__trade-head {
  border-block: 1px solid var(--line);
  color: var(--muted);
  font-size: 10px;
  min-height: 34px;
}

.market-detail__trade-head span,
.market-detail__trade span {
  align-self: center;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__trade-head span:nth-child(n + 2),
.market-detail__trade span:nth-child(n + 2) {
  text-align: right;
}

.market-detail__trade {
  border-bottom: 1px solid color-mix(in srgb, var(--line) 72%, transparent);
  color: var(--muted-strong);
  font-size: 11px;
  min-height: 32px;
}

.market-detail__trade-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  justify-content: center;
  min-height: 150px;
  padding: 20px;
}

.market-detail__actions {
  background: var(--surface);
  border-top: 1px solid var(--line);
  bottom: 0;
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  left: 50%;
  max-width: var(--app-max-width);
  padding: 8px 12px calc(8px + env(safe-area-inset-bottom));
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: var(--layer-navigation);
}

.market-detail__actions button {
  align-items: center;
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 36%, var(--line));
  color: var(--accent);
  display: flex;
  font-size: 12px;
  font-weight: 800;
  gap: 7px;
  justify-content: center;
  min-height: 48px;
  min-width: 0;
  padding: 0 8px;
}

.market-detail__actions .is-primary {
  background: var(--positive);
  border-color: var(--positive);
  color: var(--on-positive);
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 360px) {
  .market-detail__price {
    padding-left: 16px;
    padding-right: 16px;
  }

  .market-detail__error {
    margin-left: 16px;
    margin-right: 16px;
  }

  .market-detail__chart-panel > header,
  .market-detail__book :deep(.order-book),
  .market-detail__trades > header,
  .market-detail__trade-head,
  .market-detail__trade {
    padding-left: 16px;
    padding-right: 16px;
  }
}

@media (max-width: 340px) {
  .market-detail__header {
    padding-left: 6px;
    padding-right: 6px;
  }

  .market-detail__instrument {
    gap: 6px;
  }

  .market-detail__price {
    gap: 10px;
    grid-template-columns: minmax(0, 1fr) 114px;
    padding-left: 14px;
    padding-right: 14px;
  }

  .market-detail__quote > strong {
    font-size: 27px;
  }

  .market-detail__error {
    margin-left: 14px;
    margin-right: 14px;
  }

  .market-detail__chart-panel > header,
  .market-detail__book :deep(.order-book),
  .market-detail__trades > header,
  .market-detail__trade-head,
  .market-detail__trade {
    padding-left: 14px;
    padding-right: 14px;
  }

  .market-detail__chart {
    height: 278px;
  }

  .market-detail__trade-head,
  .market-detail__trade {
    gap: 7px;
    grid-template-columns: minmax(0, 1fr) 70px 54px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
