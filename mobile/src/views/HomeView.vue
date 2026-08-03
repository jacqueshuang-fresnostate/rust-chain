<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownLeft,
  ArrowRight,
  CandlestickChart,
  ChevronRight,
  CircleDollarSign,
  CreditCard,
  Eye,
  EyeOff,
  Grid2X2,
  Landmark,
  Layers3,
  Newspaper,
  Repeat2,
  Rocket,
  ScanQrCode,
  Search,
  Zap,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import guestHeroDark from '@/assets/home/market-hero-dark.jpg'
import guestHeroLight from '@/assets/home/market-hero-light.jpg'
import { fetchNews } from '@/api/news'
import { fetchMarginWallets } from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import type { NewsItem, WalletAccount } from '@/core/types'

const router = useRouter()
const marketStore = useMarketStore()
const navigation = useNavigationStore()
const session = useSessionStore()
const { locale, t } = useI18n()

type HomeTab = 'favorites' | 'mainstream' | 'popular' | 'gainers' | 'newCoins'
type TradeMode = 'spot' | 'contract'

const activeTab = ref<HomeTab>('popular')
const assetVisible = ref(true)
const announcements = ref<NewsItem[]>([])
const announcementState = ref<'loading' | 'ready' | 'empty' | 'error'>('loading')
const spotAccounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const assetEstimateReady = ref(false)
const portfolioSamples = ref<number[]>([])

const tabs = computed(() => [
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'mainstream' as const, label: t('home.mainstream') },
  { key: 'popular' as const, label: t('home.popular') },
  { key: 'gainers' as const, label: t('home.gainers') },
  { key: 'newCoins' as const, label: t('products.newCoins') },
])

const portfolioPeriods = computed(() => [1, 7, 30, 180].map((days) => ({
  days,
  label: t('home.periodDays', { days }),
})))

const visibleTickers = computed(() => {
  const rows = [...marketStore.topTickers]
  if (activeTab.value === 'favorites') return []
  if (activeTab.value === 'gainers') return rows.sort((left, right) => right.changePercent - left.changePercent)
  if (activeTab.value === 'popular' || activeTab.value === 'mainstream') return rows.sort((left, right) => right.volume - left.volume)
  if (activeTab.value === 'newCoins') return rows.reverse()
  return rows
})
const marketRowsUnavailable = computed(() => (
  marketStore.error
  || (!marketStore.updatedAt && !marketStore.tickers.length)
))

const briefNotice = computed<NewsItem | null>(() => announcements.value[0] || null)
const briefMessage = computed(() => {
  if (briefNotice.value) return briefNotice.value.title
  if (announcementState.value === 'loading') return t('home.announcementLoading')
  if (announcementState.value === 'error') return t('home.announcementUnavailable')
  return t('home.announcementEmpty')
})

const totalAssetEstimate = computed(() => [...spotAccounts.value, ...marginAccounts.value].reduce((total, account) => {
  const accountAmount = account.available + account.frozen + account.locked
  if (account.symbol === 'USDT' || account.symbol === 'USDC' || account.symbol === 'USD') return total + accountAmount
  return total + accountAmount * (marketStore.tickerFor(`${account.symbol}/USDT`)?.lastPrice || 0)
}, 0))

const assetEstimateComplete = computed(() => [...spotAccounts.value, ...marginAccounts.value].every((account) => {
  const accountAmount = account.available + account.frozen + account.locked
  return accountAmount === 0
    || account.symbol === 'USDT'
    || account.symbol === 'USDC'
    || account.symbol === 'USD'
    || Boolean(marketStore.tickerFor(`${account.symbol}/USDT`))
}))

const displayedAssetAmount = computed(() => (
  session.isAuthenticated && assetEstimateReady.value && assetEstimateComplete.value
    ? formatAmount(totalAssetEstimate.value)
    : '--'
))

const portfolioGeometry = computed(() => {
  const values = portfolioSamples.value
  if (values.length < 2) return null
  const width = 358
  const height = 153
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  const range = maximum - minimum || Math.max(Math.abs(maximum) * .005, 1)
  const points = values.map((value, index) => ({
    x: (index / (values.length - 1)) * width,
    y: 12 + ((maximum - value) / range) * (height - 24),
  }))
  return {
    path: points.map((point, index) => `${index ? 'L' : 'M'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`).join(' '),
    latest: points.at(-1)!,
  }
})

function openMarket(symbol: string): void {
  void router.push({ name: 'market-detail', params: { symbol: symbol.replace('/', '_') } })
}

function openTrade(mode: TradeMode = 'spot', symbol = 'BTC/USDT'): void {
  const routeSymbol = symbol.replace('/', '_')
  navigation.rememberTradeSymbol(routeSymbol)
  navigation.rememberTradeMode(mode)
  void router.replace({
    name: 'trade',
    params: { symbol: routeSymbol },
    query: mode === 'contract' ? { mode } : undefined,
  })
}

async function loadAnnouncements(): Promise<void> {
  announcementState.value = 'loading'
  try {
    const items = await fetchNews()
    announcements.value = items
    announcementState.value = items.length ? 'ready' : 'empty'
  } catch {
    announcements.value = []
    announcementState.value = 'error'
  }
}

function openBriefNotice(): void {
  if (!briefNotice.value) return
  void router.push({ name: 'news-detail', params: { id: String(briefNotice.value.id) } })
}

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: '/' } })
}

async function loadAssetEstimate(): Promise<void> {
  if (!session.isAuthenticated) {
    spotAccounts.value = []
    marginAccounts.value = []
    assetEstimateReady.value = false
    return
  }

  assetEstimateReady.value = false
  try {
    const [nextSpotAccounts, marginState] = await Promise.all([
      fetchWalletAccounts(),
      fetchMarginWallets(),
    ])
    spotAccounts.value = nextSpotAccounts
    marginAccounts.value = marginState.wallets
    assetEstimateReady.value = true
  } catch {
    spotAccounts.value = []
    marginAccounts.value = []
  }
}

async function refreshMarkets(force = false): Promise<void> {
  await marketStore.refresh(force)
  marketStore.startLiveUpdates()
}

onMounted(async () => {
  void loadAnnouncements()
  await refreshMarkets()
})
onUnmounted(() => marketStore.stopLiveUpdates())
watch(locale, () => { void loadAnnouncements() })
watch(() => session.isAuthenticated, () => { void loadAssetEstimate() }, { immediate: true })
watch(
  [
    () => session.isAuthenticated,
    assetEstimateReady,
    assetEstimateComplete,
    totalAssetEstimate,
  ],
  ([authenticated, ready, complete, value]) => {
    if (!authenticated) {
      portfolioSamples.value = []
      return
    }
    if (!ready || !complete || !Number.isFinite(value)) return
    if (portfolioSamples.value.at(-1) === value) return
    portfolioSamples.value = [...portfolioSamples.value, value].slice(-32)
  },
  { immediate: true },
)
</script>

<template>
  <main class="view home-view prototype-root-view">
    <section class="home-workspace">
      <div class="home-utility-row">
        <button class="home-search" type="button" @click="router.replace({ name: 'markets' })">
          <Search :size="17" aria-hidden="true" />
          <span>{{ t('home.searchPlaceholder') }}</span>
        </button>
        <div class="home-utility-actions action-cluster" role="group" :aria-label="t('home.scan')">
          <button
            class="utility-icon"
            type="button"
            :aria-label="t('home.scan')"
            @click="router.push({ name: 'deposit-asset' })"
          >
            <ScanQrCode :size="18" aria-hidden="true" />
          </button>
        </div>
      </div>
    </section>

    <section v-if="!session.isAuthenticated" class="home-portfolio home-portfolio--guest">
      <article class="home-guest-hero">
        <img class="home-guest-hero__image home-guest-hero__image--light" :src="guestHeroLight" alt="">
        <img class="home-guest-hero__image home-guest-hero__image--dark" :src="guestHeroDark" alt="">
        <span class="home-guest-hero__overlay" aria-hidden="true" />
        <span class="home-guest-hero__bloom" aria-hidden="true" />
        <div class="home-guest-hero__copy">
          <h1>
            <span>{{ t('home.guestHeroLine1') }}</span>
            <span>{{ t('home.guestHeroLine2') }}</span>
          </h1>
          <p>{{ t('home.guestHeroDescription') }}</p>
        </div>
        <button class="home-guest-hero__login" type="button" @click="openLogin">
          {{ t('home.guestHeroLogin') }}
          <ArrowRight :size="18" aria-hidden="true" />
        </button>
      </article>
    </section>

    <section
      v-else
      class="portfolio-overview home-portfolio home-portfolio--member"
      data-portfolio-source="live-wallet-estimate"
      :aria-busy="!assetEstimateReady"
    >
        <div class="portfolio-heading">
          <div>
            <div class="balance-label">
              <span>{{ t('home.totalAssetValue') }}</span>
              <button
                class="inline-icon"
                type="button"
                :aria-label="t('home.assetOverview')"
                :aria-pressed="!assetVisible"
                @click="assetVisible = !assetVisible"
              >
                <Eye v-if="assetVisible" :size="15" aria-hidden="true" />
                <EyeOff v-else :size="15" aria-hidden="true" />
              </button>
            </div>
            <strong class="portfolio-balance numeric">
              {{ assetVisible ? displayedAssetAmount : '••••••' }}
              <small> USDT</small>
            </strong>
          </div>
          <div class="portfolio-change">
            <span>{{ t('rootPrototype.todayReturn') }}</span>
            <strong class="numeric">--</strong>
            <small class="numeric">--</small>
          </div>
        </div>
        <div
          class="portfolio-chart"
          :class="{ 'has-live-history': portfolioGeometry }"
          :aria-label="t('home.assetOverview')"
        >
          <svg viewBox="0 0 358 153" preserveAspectRatio="none" aria-hidden="true">
            <path
              v-if="portfolioGeometry"
              :d="portfolioGeometry.path"
              fill="none"
              stroke="var(--signal-green)"
              stroke-width="2"
              vector-effect="non-scaling-stroke"
            />
            <circle
              v-if="portfolioGeometry"
              :cx="portfolioGeometry.latest.x"
              :cy="portfolioGeometry.latest.y"
              r="5"
              fill="var(--surface)"
              stroke="var(--signal-green)"
              stroke-width="2"
              vector-effect="non-scaling-stroke"
            />
          </svg>
        </div>
        <div class="portfolio-periods" role="list" :aria-label="t('home.assetOverview')">
          <span
            v-for="period in portfolioPeriods"
            :key="period.days"
            role="listitem"
            :class="{ active: period.days === 1 }"
          >
            {{ period.label }}
          </span>
        </div>
    </section>

    <section class="funding-actions" :aria-label="t('assets.operations')">
      <button class="buy-crypto" type="button" @click="router.push({ name: 'quick-recharge' })">
        <CreditCard :size="18" aria-hidden="true" />
        {{ t('rootPrototype.buyCrypto') }}
      </button>
      <button class="deposit-crypto" type="button" @click="router.push({ name: 'deposit-asset' })">
        <ArrowDownLeft :size="18" aria-hidden="true" />
        {{ t('home.deposit') }}
      </button>
    </section>

    <section class="shortcut-section" :aria-label="t('home.productCenter')">
      <div class="shortcut-grid">
        <button type="button" @click="router.push({ name: 'swap' })"><span><Repeat2 :size="20" /></span><small>{{ t('trade.swap') }}</small></button>
        <button type="button" @click="openTrade('spot')"><span><CandlestickChart :size="20" /></span><small>{{ t('trade.spot') }}</small></button>
        <button type="button" @click="openTrade('contract')"><span><Layers3 :size="20" /></span><small>{{ t('trade.contract') }}</small></button>
        <button type="button" @click="router.push({ name: 'earn' })"><span><Landmark :size="20" /></span><small>{{ t('products.earn') }}</small></button>
        <button type="button" @click="router.push({ name: 'loan' })"><span><CircleDollarSign :size="20" /></span><small>{{ t('products.loan') }}</small></button>
        <button type="button" @click="router.push({ name: 'new-coins' })"><span><Rocket :size="20" /></span><small>{{ t('home.newCoinsShortcut') }}</small></button>
        <button type="button" data-home-shortcut="seconds" @click="router.push({ name: 'seconds' })"><span><Zap :size="19" /></span><small>{{ t('home.secondsShortcut') }}</small></button>
        <button type="button" @click="router.push({ name: 'products' })"><span><Grid2X2 :size="20" /></span><small>{{ t('common.more') }}</small></button>
      </div>
    </section>

    <button
      class="market-brief"
      type="button"
      :aria-busy="announcementState === 'loading'"
      :disabled="!briefNotice"
      @click="openBriefNotice"
    >
      <span class="brief-icon"><Newspaper :size="20" aria-hidden="true" /></span>
      <span>
        <small>{{ t('rootPrototype.aiMarketBrief') }}</small>
        <strong>{{ t('rootPrototype.aiMarketBriefTitle') }}</strong>
        <em>{{ briefMessage }}</em>
      </span>
      <ChevronRight :size="17" aria-hidden="true" />
    </button>

    <section class="home-market-section">
      <div class="section-heading">
        <div><h2>{{ t('markets.title') }}</h2></div>
        <button class="text-action" type="button" @click="router.replace({ name: 'markets' })">
          {{ t('common.more') }} <ChevronRight :size="15" />
        </button>
      </div>
      <div class="home-market-tabs" role="tablist" :aria-label="t('home.marketSummary')">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.key"
          :class="{ active: activeTab === tab.key }"
          @click="activeTab = tab.key"
        >
          {{ tab.label }}
        </button>
      </div>
      <div class="home-market-head" aria-hidden="true">
        <span>{{ t('markets.pair') }}</span>
        <span>{{ t('markets.latestPrice') }}</span>
        <span>{{ t('markets.change24h') }}</span>
      </div>
      <div
        class="home-market-list"
        :class="{ 'root-market-reserved': marketRowsUnavailable }"
        :aria-busy="marketRowsUnavailable && !marketStore.error"
        :aria-label="marketRowsUnavailable ? t(marketStore.error ? 'common.marketLoadFailed' : 'common.loading') : undefined"
      >
        <template v-if="marketRowsUnavailable">
          <div v-for="row in 3" :key="`home-market-skeleton-${row}`" class="home-market-skeleton-row" aria-hidden="true">
            <span class="home-market-name">
              <span class="coin-orbit root-skeleton" />
              <span>
                <i class="root-market-skeleton-block root-skeleton" />
                <i class="root-market-skeleton-block compact root-skeleton" />
              </span>
            </span>
            <span class="home-market-price">
              <i class="root-market-skeleton-block root-skeleton" />
              <i class="root-market-skeleton-block compact root-skeleton" />
            </span>
            <span class="market-change root-skeleton" />
          </div>
          <div v-if="marketStore.error" class="root-market-reserved-state" role="alert">
            <span>{{ t('common.marketLoadFailed') }}</span>
            <button type="button" :disabled="marketStore.loading" @click="refreshMarkets(true)">{{ t('common.retry') }}</button>
          </div>
        </template>

        <template v-else>
          <button
            v-for="ticker in visibleTickers.slice(0, 3)"
            :key="ticker.symbol"
            type="button"
            @click="openMarket(ticker.symbol)"
          >
            <span class="home-market-name">
              <span class="coin-orbit">
                <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :size="32" />
              </span>
              <span><strong>{{ ticker.base }}</strong><small>/{{ ticker.quote }}</small></span>
            </span>
            <span class="home-market-price">
              <strong class="numeric">{{ formatPrice(ticker.lastPrice) }}</strong>
              <small>{{ ticker.quote }}</small>
            </span>
            <span
              class="market-change numeric"
              :class="ticker.changePercent >= 0 ? 'positive-bg' : 'negative-bg'"
            >
              {{ formatPercent(ticker.changePercent) }}
            </span>
          </button>

          <div v-if="!visibleTickers.length" class="root-market-empty-state">
            <Search :size="24" aria-hidden="true" />
            <strong>{{ t('markets.noResults') }}</strong>
            <span>{{ t('rootPrototype.noMarketHint') }}</span>
          </div>
        </template>
      </div>
    </section>

  </main>
</template>

<style>
.home-view .market-brief:disabled {
  opacity: 1;
}
</style>
