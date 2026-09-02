<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  ArrowDownLeft,
  ArrowRight,
  ArrowUpRight,
  CandlestickChart,
  ChevronRight,
  CircleDollarSign,
  CreditCard,
  Eye,
  EyeOff,
  Grid2X2,
  Landmark,
  Layers3,
  Repeat2,
  Rocket,
  ScanQrCode,
  Search,
  Zap,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import guestHeroDark from '@/assets/home/market-hero-dark.jpg'
import guestHeroLight from '@/assets/home/market-hero-light.jpg'
import { fetchMarginWallets } from '@/api/trading'
import {
  createReturnHistoryRequestLifecycle,
  createTodayReturnRequestLifecycle,
  fetchReturnHistory,
  fetchTodayReturn,
  fetchWalletAccounts,
  RETURN_HISTORY_PERIODS,
  type ReturnHistory,
  type ReturnHistoryPeriodDays,
  type ReturnHistoryViewState,
  type TodayReturn,
} from '@/api/wallet'
import { formatAmount, formatPercent, formatPrice } from '@/core/format'
import { buildHomeMarketBrief } from '@/core/homeMarketBrief'
import { buildReturnHistoryGeometry } from '@/core/returnHistoryGeometry'
import { decimalSign, normalizeDecimalText, type DecimalText } from '@/core/decimal'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { resolveTodayReturnPresentation } from '@/core/todayReturnPresentation'
import { useMarketStore } from '@/stores/market'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'
import type { WalletAccount } from '@/core/types'

const router = useRouter()
const marketStore = useMarketStore()
const marketFavorites = useMarketFavoritesStore()
const navigation = useNavigationStore()
const session = useSessionStore()
const theme = useThemeStore()
const { locale, t } = useI18n()

type HomeTab = 'favorites' | 'mainstream' | 'popular' | 'gainers' | 'newCoins'
type TradeMode = 'spot' | 'contract'

const activeTab = ref<HomeTab>('popular')
const assetVisible = ref(true)
const spotAccounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const assetEstimateReady = ref(false)
const todayReturn = ref<TodayReturn | null>(null)
const todayReturnState = ref<'idle' | 'loading' | 'complete' | 'partial' | 'error'>('idle')
const selectedReturnHistoryPeriod = ref<ReturnHistoryPeriodDays>(1)
const returnHistory = ref<ReturnHistory | null>(null)
const returnHistoryState = ref<ReturnHistoryViewState>('idle')
const todayReturnRequestLifecycle = createTodayReturnRequestLifecycle({
  sessionKey: () => session.token,
  fetchTodayReturn,
})
const returnHistoryRequestLifecycle = createReturnHistoryRequestLifecycle({
  sessionKey: () => session.token,
  fetchReturnHistory: () => fetchReturnHistory(selectedReturnHistoryPeriod.value),
})
let viewActive = true

const tabs = computed(() => [
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'mainstream' as const, label: t('home.mainstream') },
  { key: 'popular' as const, label: t('home.popular') },
  { key: 'gainers' as const, label: t('home.gainers') },
  { key: 'newCoins' as const, label: t('products.newCoins') },
])

const portfolioPeriods = computed(() => RETURN_HISTORY_PERIODS.map((days) => ({
  days,
  label: t('home.periodDays', { days }),
})))

const visibleTickers = computed(() => {
  const rows = activeTab.value === 'favorites'
    ? marketStore.tickers.filter((ticker) => marketFavorites.isFavorite(ticker.symbol))
    : [...marketStore.topTickers]
  if (activeTab.value === 'gainers') return rows.sort((left, right) => right.changePercent - left.changePercent)
  if (activeTab.value === 'popular' || activeTab.value === 'mainstream') return rows.sort((left, right) => right.volume - left.volume)
  if (activeTab.value === 'newCoins') return rows.reverse()
  return rows
})
const marketRowsUnavailable = computed(() => marketStore.tickers.length === 0)
const marketBrief = computed(() => buildHomeMarketBrief(marketStore.tickers))
const marketBriefToneLabel = computed(() => {
  const tone = marketBrief.value?.tone
  if (tone === 'positive') return t('home.marketBriefPositive')
  if (tone === 'negative') return t('home.marketBriefNegative')
  return t('home.marketBriefNeutral')
})
const marketBriefActionLabel = computed(() => marketBrief.value
  ? t('home.openMarketBrief')
  : t(marketStore.error ? 'home.retryMarketBrief' : 'home.marketBriefLoading'))
const guestHeroImage = computed(() => theme.isDark ? guestHeroDark : guestHeroLight)

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

const todayReturnPresentation = computed(() => resolveTodayReturnPresentation({
  visible: assetVisible.value,
  state: todayReturnState.value,
  value: todayReturn.value,
  locale: locale.value === 'en' ? 'en-US' : 'zh-CN',
  amountMask: '••••',
  detailMask: '••••',
  messages: {
    loading: t('home.todayReturnLoading'),
    partial: (assets) => t('home.todayReturnPartial', { assets }),
    partialUnknown: t('home.todayReturnPartialUnknown'),
    error: t('home.todayReturnUnavailable'),
  },
}))
const displayedTodayReturnAmount = computed(() => todayReturnPresentation.value.amount)
const displayedTodayReturnDetail = computed(() => todayReturnPresentation.value.detail)
const todayReturnTone = computed(() => todayReturnPresentation.value.tone)

const portfolioGeometry = computed(() => {
  if (!assetVisible.value || returnHistoryState.value !== 'complete' || !returnHistory.value) {
    return null
  }
  return buildReturnHistoryGeometry(returnHistory.value)
})

const returnHistoryStatusMessage = computed(() => {
  if (!assetVisible.value) return t('home.returnHistoryHidden')
  if (returnHistoryState.value === 'loading') return t('home.returnHistoryLoading')
  if (returnHistoryState.value === 'partial') return t('home.returnHistoryPartial')
  if (returnHistoryState.value === 'error') return t('home.returnHistoryUnavailable')
  return ''
})

const returnHistoryChartLabel = computed(() => {
  if (returnHistoryStatusMessage.value) return returnHistoryStatusMessage.value
  const value = returnHistory.value
  if (returnHistoryState.value !== 'complete' || value?.status !== 'complete') {
    return t('home.returnHistoryUnavailable')
  }
  return t('home.returnHistoryChartSummary', {
    days: value.periodDays,
    amount: formatSignedReturnAmount(value.summary.amount),
    asset: value.reportingAsset,
  })
})

const accessibleReturnHistoryPoints = computed(() => {
  const value = returnHistory.value
  if (!assetVisible.value || returnHistoryState.value !== 'complete' || value?.status !== 'complete') {
    return []
  }
  return value.points
})

function formatSignedReturnAmount(value: DecimalText | null | undefined): string {
  const decimal = value ?? normalizeDecimalText('0')
  const formatted = formatFinancialAmount(decimal, locale.value === 'en' ? 'en-US' : 'zh-CN', {
    assetSymbol: 'USDT',
  })
  return `${decimalSign(decimal) > 0 && !formatted.startsWith('<') ? '+' : ''}${formatted}`
}

function formatReturnHistoryDay(dayStartAt: number): string {
  return new Intl.DateTimeFormat(locale.value, {
    timeZone: 'UTC',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(new Date(dayStartAt))
}

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

function openMarketBrief(): void {
  if (!marketBrief.value) {
    void refreshMarkets(true)
    return
  }
  void router.replace({ name: 'markets' })
}

function marketChangeClass(value: number): 'positive' | 'negative' | '' {
  if (value > 0) return 'positive'
  if (value < 0) return 'negative'
  return ''
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

async function loadTodayReturn(): Promise<void> {
  if (!session.token) {
    todayReturn.value = null
    todayReturnState.value = 'idle'
    return
  }

  todayReturn.value = null
  todayReturnState.value = 'loading'
  const result = await todayReturnRequestLifecycle.load()
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    todayReturn.value = null
    todayReturnState.value = 'idle'
    return
  }
  if (result.state === 'loaded') {
    todayReturn.value = result.value
    todayReturnState.value = result.value.status
    return
  }
  todayReturn.value = null
  todayReturnState.value = 'error'
}

async function loadReturnHistory(): Promise<void> {
  if (!session.token) {
    returnHistory.value = null
    returnHistoryState.value = 'idle'
    return
  }

  returnHistory.value = null
  returnHistoryState.value = 'loading'
  const result = await returnHistoryRequestLifecycle.load()
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    returnHistory.value = null
    returnHistoryState.value = 'idle'
    return
  }
  if (result.state === 'loaded') {
    returnHistory.value = result.value
    returnHistoryState.value = result.value.status
    return
  }
  returnHistory.value = null
  returnHistoryState.value = 'error'
}

function selectReturnHistoryPeriod(periodDays: ReturnHistoryPeriodDays): void {
  if (periodDays === selectedReturnHistoryPeriod.value) return
  selectedReturnHistoryPeriod.value = periodDays
  returnHistoryRequestLifecycle.invalidate()
  void loadReturnHistory()
}

function retryReturnHistory(): void {
  returnHistoryRequestLifecycle.invalidate()
  void loadReturnHistory()
}

async function refreshMarkets(force = false): Promise<void> {
  await marketStore.refresh(force)
  if (viewActive) marketStore.startLiveUpdates('home')
}

onMounted(async () => {
  await refreshMarkets()
})
onUnmounted(() => {
  viewActive = false
  todayReturnRequestLifecycle.stop()
  returnHistoryRequestLifecycle.stop()
  marketStore.stopLiveUpdates('home')
})
watch(() => session.isAuthenticated, () => { void loadAssetEstimate() }, { immediate: true })
watch(() => session.token, () => {
  todayReturnRequestLifecycle.invalidate()
  void loadTodayReturn()
}, { immediate: true })
watch(() => session.token, () => {
  returnHistoryRequestLifecycle.invalidate()
  returnHistory.value = null
  returnHistoryState.value = 'idle'
  selectedReturnHistoryPeriod.value = 1
  void loadReturnHistory()
}, { immediate: true })
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
        <img class="home-guest-hero__image" :src="guestHeroImage" alt="">
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
      data-portfolio-source="realized-return-history"
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
          <div
            class="portfolio-change"
            :data-today-return-status="todayReturnState"
            :aria-busy="todayReturnState === 'loading'"
            aria-live="polite"
          >
            <span>{{ t('rootPrototype.todayReturn') }}</span>
            <strong class="numeric" :class="todayReturnTone">{{ displayedTodayReturnAmount }}</strong>
            <small class="numeric" :class="todayReturnTone">{{ displayedTodayReturnDetail }}</small>
          </div>
        </div>
        <figure
          id="portfolio-return-history-chart"
          class="portfolio-chart"
          :class="{ 'has-live-history': portfolioGeometry }"
          :data-return-history-status="assetVisible ? returnHistoryState : 'hidden'"
          :aria-busy="assetVisible && returnHistoryState === 'loading'"
          :aria-label="returnHistoryChartLabel"
        >
          <svg viewBox="0 0 358 153" preserveAspectRatio="none" aria-hidden="true">
            <path
              v-if="portfolioGeometry"
              :d="portfolioGeometry.path"
              class="portfolio-return-line"
              :class="portfolioGeometry.tone"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              vector-effect="non-scaling-stroke"
            />
            <circle
              v-if="portfolioGeometry"
              :cx="portfolioGeometry.latest.x"
              :cy="portfolioGeometry.latest.y"
              class="portfolio-return-dot"
              :class="portfolioGeometry.tone"
              r="5"
              fill="var(--surface)"
              stroke="currentColor"
              stroke-width="2"
              vector-effect="non-scaling-stroke"
            />
          </svg>
          <div
            v-if="returnHistoryStatusMessage"
            class="portfolio-history-state"
            :role="returnHistoryState === 'error' && assetVisible ? 'alert' : 'status'"
            aria-live="polite"
          >
            <span>{{ returnHistoryStatusMessage }}</span>
            <button
              v-if="assetVisible && (returnHistoryState === 'partial' || returnHistoryState === 'error')"
              type="button"
              @click="retryReturnHistory"
            >
              {{ t('common.retry') }}
            </button>
          </div>
          <figcaption v-if="accessibleReturnHistoryPoints.length" class="sr-only">
            {{ returnHistoryChartLabel }}
          </figcaption>
          <table v-if="accessibleReturnHistoryPoints.length" class="sr-only">
            <caption>{{ t('home.returnHistoryTableCaption') }}</caption>
            <thead>
              <tr>
                <th scope="col">{{ t('home.returnHistoryDate') }}</th>
                <th scope="col">{{ t('home.returnHistoryDaily') }}</th>
                <th scope="col">{{ t('home.returnHistoryCumulative') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="point in accessibleReturnHistoryPoints" :key="point.dayStartAt">
                <th scope="row">{{ formatReturnHistoryDay(point.dayStartAt) }}</th>
                <td>{{ formatSignedReturnAmount(point.amount) }} USDT</td>
                <td>{{ formatSignedReturnAmount(point.cumulativeAmount) }} USDT</td>
              </tr>
            </tbody>
          </table>
        </figure>
        <div class="portfolio-periods" role="group" :aria-label="t('home.returnHistoryPeriodLabel')">
          <button
            v-for="period in portfolioPeriods"
            :key="period.days"
            type="button"
            :class="{ active: period.days === selectedReturnHistoryPeriod }"
            :aria-pressed="period.days === selectedReturnHistoryPeriod"
            aria-controls="portfolio-return-history-chart"
            @click="selectReturnHistoryPeriod(period.days)"
          >
            {{ period.label }}
          </button>
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
      :data-tone="marketBrief?.tone || (marketStore.error ? 'negative' : 'loading')"
      :aria-busy="!marketBrief && !marketStore.error"
      :aria-label="marketBriefActionLabel"
      @click="openMarketBrief"
    >
      <span class="market-brief__wash" aria-hidden="true" />
      <span class="market-brief__topline">
        <span class="market-brief__identity">
          <Activity :size="15" aria-hidden="true" />
          <small>{{ t('home.marketBriefTitle') }}</small>
          <i aria-hidden="true" />
          <em>{{ t('home.marketBriefLive') }}</em>
        </span>
        <span v-if="marketBrief" class="market-brief__count">
          {{ t('home.marketBriefMarketCount', { count: marketBrief.total }) }}
        </span>
        <span class="market-brief__open" aria-hidden="true">
          <ArrowUpRight :size="16" />
        </span>
      </span>

      <template v-if="marketBrief">
        <span class="market-brief__hero">
          <span class="market-brief__signal">
            <strong>{{ marketBriefToneLabel }}</strong>
            <small>
              {{ t('home.marketBriefBreadthDetail', {
                rising: marketBrief.rising,
                falling: marketBrief.falling,
                unchanged: marketBrief.unchanged,
              }) }}
            </small>
          </span>
          <span class="market-brief__breadth">
            <small>{{ t('home.marketBriefAdvancing') }}</small>
            <strong class="numeric">{{ marketBrief.advancingPercent }}<em>%</em></strong>
          </span>
        </span>

        <span class="market-brief__meter" aria-hidden="true">
          <i :style="{ width: `${marketBrief.advancingPercent}%` }" />
        </span>

        <span class="market-brief__quotes">
          <span class="market-brief__quote">
            <small>{{ marketBrief.focusTicker.symbol }}</small>
            <span>
              <strong class="numeric">{{ formatPrice(marketBrief.focusTicker.lastPrice) }}</strong>
              <em class="numeric" :class="marketChangeClass(marketBrief.focusTicker.changePercent)">
                {{ formatPercent(marketBrief.focusTicker.changePercent) }}
              </em>
            </span>
          </span>
          <span class="market-brief__quote market-brief__quote--mover">
            <small>{{ t('home.marketBriefTopMover') }}</small>
            <span>
              <strong>{{ marketBrief.topMover.base }}</strong>
              <em class="numeric" :class="marketChangeClass(marketBrief.topMover.changePercent)">
                {{ formatPercent(marketBrief.topMover.changePercent) }}
              </em>
            </span>
          </span>
        </span>
      </template>

      <span v-else class="market-brief__state" :role="marketStore.error ? 'alert' : 'status'">
        <i aria-hidden="true" />
        <span>
          <strong>{{ t(marketStore.error ? 'home.marketBriefUnavailable' : 'home.marketBriefLoading') }}</strong>
          <small>{{ t(marketStore.error ? 'home.marketBriefTapToRetry' : 'home.marketBriefWaiting') }}</small>
        </span>
      </span>
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
                <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :fallback-src="ticker.baseIconUrl" :size="32" />
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
