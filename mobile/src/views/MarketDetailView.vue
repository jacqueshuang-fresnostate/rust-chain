<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeft, ArrowLeftRight, BellRing, ChartNoAxesCombined, Grid2X2, Share2, Star } from 'lucide-vue-next'
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

async function load(): Promise<void> {
  const version = ++requestVersion
  loading.value = true
  void marketStore.refresh()
  const [klineResult, depthResult, tradesResult] = await Promise.allSettled([
    fetchKlines(pairSymbol.value, interval.value),
    fetchOrderBook(pairSymbol.value),
    fetchRecentTrades(pairSymbol.value),
  ])
  if (version !== requestVersion) return

  const hasKlines = klineResult.status === 'fulfilled' && klineResult.value.length > 0
  const hasDepth = depthResult.status === 'fulfilled' && (depthResult.value.bids.length > 0 || depthResult.value.asks.length > 0)
  const hasTrades = tradesResult.status === 'fulfilled' && tradesResult.value.length > 0
  // 仅在请求真正失败时报错；合法的空盘口/无成交不应显示加载失败。
  dataError.value = [klineResult, depthResult, tradesResult].some((result) => result.status === 'rejected')
  points.value = hasKlines ? klineResult.value : []
  bids.value = hasDepth ? depthResult.value.bids : []
  asks.value = hasDepth ? depthResult.value.asks : []
  trades.value = hasTrades ? tradesResult.value : []
  loading.value = false
}

function retry(): void {
  void marketStore.refresh(true)
  void load()
}

function chooseInterval(value: string) {
  interval.value = value
  void load()
}

function openTrade() {
  void router.replace({ name: 'trade', params: { symbol: pairSymbol.value.replace('/', '_') } })
}

function goBack(): void {
  void goBackOr(router, { name: 'markets' })
}

watch(() => props.symbol, () => { void load() }, { immediate: true })
</script>

<template>
  <main class="market-detail">
    <header class="market-detail__header">
      <button class="icon-button" type="button" :aria-label="t('common.back')" @click="goBack"><ArrowLeft :size="26" /></button>
      <div><strong>{{ baseAsset }}/{{ quoteAsset }}</strong><span>{{ t('marketDetail.spot') }}</span></div>
      <div class="market-detail__header-actions"><button class="icon-button" type="button" :aria-label="t('marketDetail.favorite')"><Star :size="23" /></button><button class="icon-button" type="button" :aria-label="t('marketDetail.share')"><Share2 :size="22" /></button></div>
    </header>

    <nav class="market-detail__tabs" :aria-label="t('marketDetail.details')"><button class="is-active" type="button">{{ t('marketDetail.market') }}</button><button type="button">{{ t('marketDetail.overview') }}</button><button type="button">{{ t('marketDetail.data') }}</button><button type="button">{{ t('marketDetail.updates') }}</button><button type="button" @click="openTrade">{{ t('marketDetail.trade') }}</button></nav>

    <section class="market-detail__price">
      <div v-if="ticker"><span>{{ t('marketDetail.latestPrice') }}</span><strong :class="ticker.changePercent >= 0 ? 'up' : 'down'">{{ formatPrice(latestPrice) }}</strong><p>≈ {{ formatPrice(latestPrice) }} USD <b :class="ticker.changePercent >= 0 ? 'up' : 'down'">{{ formatPercent(ticker.changePercent) }}</b></p></div>
      <div v-else><span>{{ t('marketDetail.latestPrice') }}</span><strong>--</strong><p>{{ t('common.marketUnavailable') }}</p></div>
      <dl><div><dt>{{ t('marketDetail.high24h') }}</dt><dd>{{ ticker ? formatPrice(ticker.highPrice) : '--' }}</dd></div><div><dt>{{ t('marketDetail.low24h') }}</dt><dd>{{ ticker ? formatPrice(ticker.lowPrice) : '--' }}</dd></div><div><dt>{{ t('marketDetail.volume24h') }}</dt><dd>{{ ticker ? `${formatAmount(ticker.volume)} ${baseAsset}` : '--' }}</dd></div></dl>
    </section>

    <div v-if="dataError || marketStore.error" class="market-detail__error">
      <span>{{ t('common.marketLoadFailed') }}</span>
      <button type="button" :disabled="loading" @click="retry">{{ t('common.retry') }}</button>
    </div>
    <div class="market-detail__intervals"><button v-for="item in ['1m', '15m', '1h', '4h', '1d']" :key="item" type="button" :class="{ 'is-active': interval === item }" @click="chooseInterval(item)">{{ item }}</button><span>{{ t('marketDetail.indicators') }}</span></div>
    <section class="market-detail__chart"><MobileMarketChart :points="points" /><p v-if="loading" class="market-detail__loading">{{ t('marketDetail.loadingChart') }}</p></section>

    <section class="market-detail__book"><div class="market-detail__section-title"><strong>{{ t('orderBook.title') }}</strong><span>{{ t('marketDetail.depth') }}</span><span>{{ t('marketDetail.latestTrades') }}</span></div><OrderBookPanel :bids="bids" :asks="asks" :current-price="latestPrice" /></section>
    <section class="market-detail__trades"><div class="market-detail__section-title"><strong>{{ t('marketDetail.latestTrades') }}</strong><span>{{ t('marketDetail.price') }}</span><span>{{ t('marketDetail.quantity') }}</span></div><div v-for="trade in trades.slice(0, 6)" :key="trade.id" class="market-detail__trade"><span :class="trade.side === 'buy' ? 'up' : 'down'">{{ formatPrice(trade.price) }}</span><span>{{ formatAmount(trade.quantity) }}</span><span>{{ new Date(trade.time).toLocaleTimeString(currentIntlLocale(), { hour: '2-digit', minute: '2-digit' }) }}</span></div></section>

    <nav class="market-detail__actions" :aria-label="t('marketDetail.actions')"><button class="is-primary" type="button" @click="openTrade"><ArrowLeftRight :size="23" /><span>{{ t('marketDetail.trade') }}</span></button><button type="button"><Grid2X2 :size="22" /><span>{{ t('marketDetail.grid') }}</span></button><button type="button" @click="openTrade"><ChartNoAxesCombined :size="22" /><span>{{ t('marketDetail.contract') }}</span></button><button type="button"><BellRing :size="22" /><span>{{ t('marketDetail.alert') }}</span></button></nav>
  </main>
</template>

<style scoped>
.market-detail { --background: #101213; --surface: #101213; --soft: #1a1e20; --line: #252a2d; --ink: #f5f7f8; --muted: #969da4; background: #101213; color: #f5f7f8; min-height: 100dvh; padding: env(safe-area-inset-top) 0 calc(82px + env(safe-area-inset-bottom)); }
.market-detail__header { align-items: center; display: grid; grid-template-columns: 44px 1fr auto; min-height: 62px; padding: 0 12px; }.market-detail__header .icon-button { color: #f5f7f8; }.market-detail__header strong { font-size: 23px; line-height: 1; }.market-detail__header span { background: #292e31; border-radius: 5px; color: #d8dde0; font-size: 12px; font-weight: 700; margin-left: 8px; padding: 4px 6px; vertical-align: 3px; }.market-detail__header-actions { display: flex; }
.market-detail__tabs { border-bottom: 1px solid var(--line); display: flex; gap: 25px; overflow-x: auto; padding: 0 20px; }.market-detail__tabs button { background: transparent; border-bottom: 3px solid transparent; color: var(--muted); flex: 0 0 auto; font-size: 16px; font-weight: 650; min-height: 47px; padding: 0; }.market-detail__tabs .is-active { border-color: white; color: white; }
.market-detail__price { display: grid; gap: 14px; grid-template-columns: 1.15fr .85fr; padding: 22px 20px 14px; }.market-detail__price > div > span { color: var(--muted); font-size: 13px; }.market-detail__price > div > strong { display: block; font-size: 34px; line-height: 1.18; margin-top: 6px; }.market-detail__price p { color: var(--muted); font-size: 13px; margin: 7px 0 0; }.market-detail__price p b { margin-left: 6px; }.market-detail__price dl { display: grid; gap: 7px; margin: 22px 0 0; }.market-detail__price dl div { display: grid; font-size: 12px; grid-template-columns: 1fr auto; }.market-detail__price dt { color: var(--muted); }.market-detail__price dd { color: #e8ebed; margin: 0; }
.market-detail__error { align-items: center; background: #2e1a19; color: #f2988f; display: flex; font-size: 12px; gap: 10px; justify-content: space-between; margin: 0 20px; padding: 8px 10px; }.market-detail__error button { background: transparent; color: #f2988f; font-weight: 750; padding: 2px 4px; }.market-detail__intervals { align-items: center; border-top: 1px solid var(--line); display: flex; gap: 20px; overflow-x: auto; padding: 11px 20px; }.market-detail__intervals button,.market-detail__intervals span { background: transparent; color: var(--muted); flex: 0 0 auto; font-size: 13px; padding: 4px 0; }.market-detail__intervals .is-active { color: white; font-weight: 700; }
.market-detail__chart { height: 330px; position: relative; }.market-detail__loading { color: var(--muted); font-size: 12px; left: 20px; margin: 0; position: absolute; top: 8px; }.market-detail__book { border-top: 1px solid var(--line); }.market-detail__section-title { align-items: center; color: var(--muted); display: grid; font-size: 12px; grid-template-columns: 1fr auto auto; gap: 18px; padding: 14px 20px; }.market-detail__section-title strong { color: white; font-size: 16px; }.market-detail__book :deep(.order-book) { padding: 0 20px 16px; }
.market-detail__trades { border-top: 1px solid var(--line); padding-bottom: 16px; }.market-detail__trade { color: #d6dadd; display: grid; font-size: 12px; grid-template-columns: 1fr 1fr auto; padding: 5px 20px; }.market-detail__trade span:nth-child(2),.market-detail__trade span:nth-child(3) { text-align: right; }
.market-detail__actions { background: #191c1e; border-top: 1px solid #2a2f32; bottom: 0; display: grid; grid-template-columns: repeat(4, 1fr); left: 50%; max-width: var(--app-max-width); padding: 9px 12px calc(9px + env(safe-area-inset-bottom)); position: fixed; transform: translateX(-50%); width: 100%; z-index: 10; }.market-detail__actions button { align-items: center; background: transparent; color: #adb4b9; display: flex; flex-direction: column; font-size: 11px; gap: 4px; min-height: 48px; }.market-detail__actions .is-primary { color: white; }.market-detail__actions .is-primary svg { color: #00bf75; }
</style>
