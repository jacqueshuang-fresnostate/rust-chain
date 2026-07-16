<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Bell, ChevronRight, Eye, Grid2X2, ScanLine, Search, Sparkles } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import { fetchNews } from '@/api/news'
import { fallbackNews, fallbackTickers } from '@/data/fallback'
import { formatCompact, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import logo from '@/assets/logo.png'
import type { NewsItem } from '@/core/types'

const router = useRouter()
const marketStore = useMarketStore()
const session = useSessionStore()
const { locale, t } = useI18n()
type HomeTab = 'favorites' | 'popular' | 'gainers'
const activeTab = ref<HomeTab>('popular')
const announcements = ref<NewsItem[]>(fallbackNews)
const usingFallbackNews = ref(true)
const tabs = computed(() => [
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'popular' as const, label: t('home.popular') },
  { key: 'gainers' as const, label: t('home.gainers') },
])
const visibleAnnouncements = computed(() => {
  if (!usingFallbackNews.value) return announcements.value
  const titles = [t('home.fallbackAnnouncement1'), t('home.fallbackAnnouncement2'), t('home.fallbackAnnouncement3')]
  return fallbackNews.map((item, index) => ({ ...item, title: titles[index] || item.title }))
})

const visibleTickers = computed(() => {
  const rows = marketStore.topTickers.length ? [...marketStore.topTickers] : [...fallbackTickers]
  if (activeTab.value === 'gainers') return rows.sort((left, right) => right.changePercent - left.changePercent)
  if (activeTab.value === 'popular') return rows.sort((left, right) => right.volume - left.volume)
  return rows
})

function openMarket(symbol: string) {
  void router.push({ name: 'market-detail', params: { symbol: symbol.replace('/', '_') } })
}

function openTrade(symbol = 'BTC/USDT') {
  void router.replace({ name: 'trade', params: { symbol: symbol.replace('/', '_') } })
}

function selectTab(tab: HomeTab) {
  activeTab.value = tab
}

async function loadAnnouncements(): Promise<void> {
  try {
    const items = await fetchNews()
    if (items.length) {
      announcements.value = items
      usingFallbackNews.value = false
      return
    }
    usingFallbackNews.value = true
  } catch {
    usingFallbackNews.value = true
  }
}

onMounted(() => {
  void marketStore.refresh()
  void loadAnnouncements()
})
watch(locale, () => { void loadAnnouncements() })
</script>

<template>
  <main class="page home-page">
    <header class="home-header">
      <button class="icon-button" type="button" :aria-label="t('home.productCenter')" @click="router.push({ name: 'products' })"><Grid2X2 :size="25" /></button>
      <img :src="logo" class="home-header__logo" alt="Hippo" />
      <div class="home-header__actions">
        <button class="icon-button" type="button" :aria-label="t('home.scan')"><ScanLine :size="23" /></button>
        <button class="icon-button" type="button" :aria-label="t('home.notifications')"><Bell :size="22" /></button>
      </div>
    </header>

    <div class="page-content">
      <button class="market-search" type="button" @click="router.replace({ name: 'markets' })">
        <Search :size="21" /><span>{{ t('home.searchPlaceholder') }}</span>
      </button>

      <section class="asset-glance" :aria-label="t('home.assetOverview')">
        <div class="asset-glance__label">{{ t('home.totalAssetValue') }} <Eye :size="18" /></div>
        <div class="asset-glance__amount"><strong class="numeric">{{ session.isAuthenticated ? '--' : '--' }} <small>USD</small></strong><span class="asset-glance__signal"><i /><i /><i /><i /><i /></span></div>
        <p>{{ session.isAuthenticated ? t('home.memberAssetHint') : t('home.guestAssetHint') }}</p>
      </section>

      <div class="quick-actions">
        <button class="button button--primary" type="button" @click="router.push({ name: 'deposit-asset' })">{{ t('home.deposit') }}</button>
        <button class="button button--primary" type="button" @click="openTrade()">{{ t('home.trade') }}</button>
      </div>

      <section class="market-pulse" :aria-label="t('home.marketSummary')">
        <div><span>{{ t('home.marketUpdates') }}</span><strong>{{ marketStore.sampleData ? t('common.demoData') : t('common.liveData') }}</strong></div>
        <Sparkles :size="23" />
      </section>

      <div class="section-heading market-heading">
        <div class="market-tabs">
          <button v-for="tab in tabs" :key="tab.key" type="button" :class="{ 'is-active': activeTab === tab.key }" @click="selectTab(tab.key)">{{ tab.label }}</button>
        </div>
        <button class="section-heading__action" type="button" @click="router.replace({ name: 'markets' })">{{ t('common.more') }} <ChevronRight :size="16" /></button>
      </div>

      <p v-if="marketStore.sampleData" class="sample-note">{{ t('common.offlineMarketNotice') }}</p>
      <div class="ticker-list">
        <button v-for="ticker in visibleTickers.slice(0, 5)" :key="ticker.symbol" class="ticker-row" type="button" @click="openMarket(ticker.symbol)">
          <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" />
          <span class="ticker-row__name"><b>{{ ticker.base }}</b><small>/ {{ ticker.quote }}</small><em>24h {{ formatCompact(ticker.volume) }}</em></span>
          <span class="ticker-row__price"><b>{{ formatPrice(ticker.lastPrice) }}</b><small>≈ {{ formatPrice(ticker.lastPrice) }} USD</small></span>
          <span class="ticker-row__change" :class="ticker.changePercent >= 0 ? 'is-up' : 'is-down'">{{ formatPercent(ticker.changePercent) }}</span>
        </button>
      </div>

      <section class="announcements">
        <div class="section-heading"><span>{{ t('home.announcements') }}</span></div>
        <button v-for="notice in visibleAnnouncements" :key="notice.id" class="announcement-row" type="button" @click="router.push({ name: 'news-detail', params: { id: notice.id } })">
          <span>{{ notice.title }}</span><ChevronRight :size="18" />
        </button>
        <button class="announcement-more" type="button" @click="router.push({ name: 'news' })">{{ t('home.allAnnouncements') }}</button>
      </section>
    </div>
  </main>
</template>

<style scoped>
.home-page { padding-top: calc(8px + env(safe-area-inset-top)); }
.home-header { align-items: center; display: grid; grid-template-columns: 88px 1fr 88px; min-height: 56px; padding: 0 12px; }.home-header > .icon-button { justify-self: start; }
.home-header__logo { height: 25px; justify-self: center; max-width: 108px; object-fit: contain; }
.home-header__actions { display: flex; justify-self: end; }
.market-search { align-items: center; background: var(--soft); border: 1px solid transparent; border-radius: 28px; color: #9ba0a6; display: flex; font-size: 15px; gap: 10px; min-height: 52px; padding: 0 18px; text-align: left; width: 100%; }.market-search:hover { border-color: #d9dfe3; }
.asset-glance { padding: 31px 0 23px; }
.asset-glance__label { align-items: center; color: var(--muted); display: flex; font-size: 15px; gap: 6px; }
.asset-glance__amount { align-items: flex-end; display: flex; justify-content: space-between; }.asset-glance strong { display: block; font-size: 44px; font-weight: 770; line-height: 1.08; margin: 11px 0 7px; }
.asset-glance strong small { font-size: 18px; font-weight: 650; }
.asset-glance p { color: var(--positive); font-size: 14px; font-weight: 600; margin: 0; }
.asset-glance__signal { align-items: flex-end; display: flex; gap: 4px; height: 39px; margin: 0 1px 10px 0; }.asset-glance__signal i { background: var(--positive); border-radius: 2px; display: block; opacity: .28; width: 5px; }.asset-glance__signal i:nth-child(1) { height: 11px; }.asset-glance__signal i:nth-child(2) { height: 21px; opacity: .52; }.asset-glance__signal i:nth-child(3) { height: 16px; opacity: .38; }.asset-glance__signal i:nth-child(4) { height: 32px; opacity: .82; }.asset-glance__signal i:nth-child(5) { height: 25px; opacity: .62; }
.quick-actions { display: grid; gap: 12px; grid-template-columns: 1fr 1fr; }
.quick-actions .button { border-radius: 24px; min-height: 50px; }
.market-pulse { align-items: center; background: var(--soft); border: 1px solid #e8ecee; border-radius: var(--radius); box-shadow: 0 7px 18px rgb(15 23 42 / 3%); display: flex; justify-content: space-between; margin-top: 28px; min-height: 86px; padding: 17px; }
.market-pulse div { display: grid; gap: 5px; }.market-pulse span { color: var(--muted); font-size: 13px; }.market-pulse strong { font-size: 18px; }.market-pulse svg { color: var(--positive); }
.market-heading { margin-top: 34px; }.market-tabs { display: flex; gap: 20px; min-width: 0; }.market-tabs button { background: transparent; color: var(--muted); font-size: 16px; padding: 0; white-space: nowrap; }.market-tabs .is-active { color: var(--ink); font-weight: 750; }
.section-heading__action { align-items: center; display: inline-flex; }.sample-note { background: #fff8e6; border-radius: 6px; color: #8a5a00; font-size: 12px; margin: 0 0 8px; padding: 7px 9px; }
.ticker-list { display: grid; }.ticker-row { align-items: center; background: transparent; border-radius: 6px; display: grid; gap: 10px; grid-template-columns: 38px minmax(0, 1.15fr) minmax(84px, .95fr) 82px; min-height: 72px; padding: 8px 0; text-align: left; width: 100%; }.ticker-row:hover { background: #f8fafb; }.ticker-row__name,.ticker-row__price { display: grid; min-width: 0; }.ticker-row b { color: var(--ink); font-size: 16px; font-variant-numeric: tabular-nums; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.ticker-row small,.ticker-row em { color: var(--muted); font-size: 12px; font-style: normal; margin-top: 4px; }.ticker-row__price { text-align: right; }.ticker-row__change { border-radius: 6px; box-shadow: inset 0 0 0 1px rgb(255 255 255 / 14%); color: white; font-size: 14px; font-weight: 720; padding: 9px 6px; text-align: center; }.ticker-row__change.is-up { background: var(--positive); }.ticker-row__change.is-down { background: var(--negative); }
.announcements { padding-bottom: 10px; }.announcement-row { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: flex; font-size: 15px; gap: 12px; justify-content: space-between; min-height: 62px; padding: 10px 0; text-align: left; width: 100%; }.announcement-row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.announcement-more { background: transparent; color: var(--accent); font-size: 13px; font-weight: 700; margin-top: 13px; padding: 6px 0; }
</style>
