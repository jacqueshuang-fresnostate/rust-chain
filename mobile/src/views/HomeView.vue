<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  ArrowDownLeft,
  Bell,
  CandlestickChart,
  ChevronRight,
  CircleDollarSign,
  CreditCard,
  Eye,
  EyeOff,
  Gauge,
  Gift,
  Grid2X2,
  Landmark,
  Layers3,
  Moon,
  Newspaper,
  Repeat2,
  Rocket,
  ScanQrCode,
  Search,
  ShieldCheck,
  Sun,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import { fetchNews } from '@/api/news'
import { fetchMarginWallets } from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { fallbackNews } from '@/data/fallback'
import { formatAmount, formatCompact, formatFiat, formatPercent, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'
import logo from '@/assets/logo.png'
import type { NewsItem, WalletAccount } from '@/core/types'

const router = useRouter()
const marketStore = useMarketStore()
const navigation = useNavigationStore()
const session = useSessionStore()
const theme = useThemeStore()
const { locale, t } = useI18n()

type HomeTab = 'favorites' | 'mainstream' | 'popular' | 'gainers' | 'newCoins'
type TradeMode = 'spot' | 'contract'

const activeTab = ref<HomeTab>('popular')
const assetVisible = ref(true)
const portfolioPeriod = ref(1)
const announcements = ref<NewsItem[]>(fallbackNews)
const usingFallbackNews = ref(true)
const spotAccounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const assetEstimateReady = ref(false)

const tabs = computed(() => [
  { key: 'favorites' as const, label: t('home.favorites') },
  { key: 'mainstream' as const, label: t('trade.spot') },
  { key: 'popular' as const, label: t('home.popular') },
  { key: 'gainers' as const, label: t('home.gainers') },
  { key: 'newCoins' as const, label: t('products.newCoins') },
])

const portfolioPeriods = computed(() => [1, 7, 30, 180].map((days) => ({
  days,
  label: t('home.periodDays', { days }),
})))

const visibleAnnouncements = computed(() => {
  if (!usingFallbackNews.value) return announcements.value
  const titles = [t('home.fallbackAnnouncement1'), t('home.fallbackAnnouncement2'), t('home.fallbackAnnouncement3')]
  return fallbackNews.map((item, index) => ({ ...item, title: titles[index] || item.title }))
})

const visibleTickers = computed(() => {
  const rows = [...marketStore.topTickers]
  if (activeTab.value === 'gainers') return rows.sort((left, right) => right.changePercent - left.changePercent)
  if (activeTab.value === 'popular' || activeTab.value === 'mainstream') return rows.sort((left, right) => right.volume - left.volume)
  if (activeTab.value === 'newCoins') return rows.reverse()
  return rows
})

const briefNotice = computed<NewsItem>(() => visibleAnnouncements.value[0] || fallbackNews[0]!)

const totalAssetEstimate = computed(() => [...spotAccounts.value, ...marginAccounts.value].reduce((total, account) => {
  const amount = account.available + account.frozen + account.locked
  if (account.symbol === 'USDT' || account.symbol === 'USDC' || account.symbol === 'USD') return total + amount
  return total + amount * (marketStore.tickerFor(`${account.symbol}/USDT`)?.lastPrice || 0)
}, 0))

const assetEstimateComplete = computed(() => [...spotAccounts.value, ...marginAccounts.value].every((account) => {
  const amount = account.available + account.frozen + account.locked
  return amount === 0
    || account.symbol === 'USDT'
    || account.symbol === 'USDC'
    || account.symbol === 'USD'
    || Boolean(marketStore.tickerFor(`${account.symbol}/USDT`))
}))

const displayedAssetEstimate = computed(() => (
  session.isAuthenticated && assetEstimateReady.value && assetEstimateComplete.value
    ? formatFiat(totalAssetEstimate.value)
    : '--'
))

const displayedAssetAmount = computed(() => (
  session.isAuthenticated && assetEstimateReady.value && assetEstimateComplete.value
    ? formatAmount(totalAssetEstimate.value)
    : '--'
))

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

function selectTab(tab: HomeTab): void {
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

onMounted(async () => {
  void loadAnnouncements()
  await marketStore.refresh()
  marketStore.startLiveUpdates()
})
onUnmounted(() => marketStore.stopLiveUpdates())
watch(locale, () => { void loadAnnouncements() })
watch(() => session.isAuthenticated, () => { void loadAssetEstimate() }, { immediate: true })
</script>

<template>
  <main class="page home-page">
    <header class="home-header">
      <button class="home-header__brand" type="button" :aria-label="t('nav.profile')" @click="router.replace({ name: 'profile' })">
        <img :src="logo" class="home-header__logo" alt="HIPPO" />
      </button>
      <div class="home-header__actions">
        <button
          class="icon-button"
          type="button"
          :aria-label="t(theme.isDark ? 'home.switchToLightTheme' : 'home.switchToDarkTheme')"
          :title="t(theme.isDark ? 'home.switchToLightTheme' : 'home.switchToDarkTheme')"
          :aria-pressed="theme.isDark"
          @click="theme.toggleTheme"
        >
          <Sun v-if="theme.isDark" :size="21" aria-hidden="true" />
          <Moon v-else :size="21" aria-hidden="true" />
        </button>
        <button class="icon-button home-header__notification" type="button" :aria-label="t('home.openMessageCenter')" @click="router.push({ name: 'message-center' })">
          <Bell :size="21" aria-hidden="true" />
        </button>
      </div>
    </header>

    <section class="home-workspace">
      <div class="home-utility-row">
        <button class="market-search" type="button" @click="router.replace({ name: 'markets' })">
          <Search :size="18" />
          <span>{{ t('home.searchPlaceholder') }}</span>
        </button>
        <button class="home-scan" type="button" :aria-label="t('home.scan')" @click="router.push({ name: 'deposit-asset' })">
          <ScanQrCode :size="19" aria-hidden="true" />
        </button>
      </div>
      <section class="asset-glance" :aria-label="t('home.assetOverview')">
        <div class="asset-glance__heading">
          <div>
            <span class="asset-glance__label">
              {{ t('home.totalAssetValue') }}
              <button
                class="asset-glance__visibility"
                type="button"
                :aria-label="t('home.assetOverview')"
                :aria-pressed="!assetVisible"
                @click="assetVisible = !assetVisible"
              >
                <Eye v-if="assetVisible" :size="16" aria-hidden="true" />
                <EyeOff v-else :size="16" aria-hidden="true" />
              </button>
            </span>
            <strong class="asset-glance__balance numeric">
              {{ assetVisible ? displayedAssetAmount : '••••••' }}
              <small>USDT</small>
            </strong>
            <span class="asset-glance__fiat numeric">{{ assetVisible ? displayedAssetEstimate : '••••••' }}</span>
          </div>
          <div class="asset-glance__return">
            <span>{{ t('markets.change24h') }}</span>
            <strong class="numeric up">--</strong>
            <small class="numeric up">--</small>
          </div>
        </div>
        <div class="asset-glance__chart" :aria-label="t('home.assetOverview')">
          <span class="asset-glance__chart-line" aria-hidden="true" />
          <i aria-hidden="true" />
        </div>
        <div class="asset-glance__periods" role="group" :aria-label="t('home.assetOverview')">
          <button
            v-for="period in portfolioPeriods"
            :key="period.days"
            type="button"
            :aria-pressed="portfolioPeriod === period.days"
            :class="{ 'is-active': portfolioPeriod === period.days }"
            @click="portfolioPeriod = period.days"
          >
            {{ period.label }}
          </button>
        </div>
      </section>
    </section>

    <section class="funding-actions" :aria-label="t('assets.operations')">
      <button class="buy-crypto" type="button" @click="router.push({ name: 'quick-recharge' })">
        <CreditCard :size="18" aria-hidden="true" />
        {{ t('assets.quickBuy') }}
      </button>
      <button class="deposit-crypto" type="button" @click="router.push({ name: 'deposit-asset' })">
        <ArrowDownLeft :size="18" aria-hidden="true" />
        {{ t('home.deposit') }}
      </button>
    </section>

    <section class="shortcut-section" :aria-label="t('home.productCenter')">
      <button type="button" @click="router.push({ name: 'swap' })"><span><Repeat2 :size="20" /></span><small>{{ t('trade.swap') }}</small></button>
      <button type="button" @click="openTrade('spot')"><span><CandlestickChart :size="20" /></span><small>{{ t('trade.spot') }}</small></button>
      <button type="button" @click="openTrade('contract')"><span><Layers3 :size="20" /></span><small>{{ t('trade.contract') }}</small></button>
      <button type="button" @click="router.push({ name: 'earn' })"><span><Landmark :size="20" /></span><small>{{ t('products.earn') }}</small></button>
      <button type="button" @click="router.push({ name: 'loan' })"><span><CircleDollarSign :size="20" /></span><small>{{ t('products.loan') }}</small></button>
      <button type="button" @click="router.push({ name: 'new-coins' })"><span><Rocket :size="20" /></span><small>{{ t('products.newCoins') }}</small></button>
      <button type="button" @click="router.push({ name: 'seconds' })"><span><Gauge :size="20" /></span><small>{{ t('seconds.title') }}</small></button>
      <button type="button" @click="router.push({ name: 'products' })"><span><Grid2X2 :size="20" /></span><small>{{ t('common.more') }}</small></button>
    </section>

    <button class="announcement-row" type="button" @click="router.push({ name: 'news-detail', params: { id: briefNotice.id } })">
      <span class="brief-icon"><Newspaper :size="20" aria-hidden="true" /></span>
      <span>
        <small>{{ t('home.marketUpdates') }}</small>
        <strong>{{ t('home.marketSummary') }}</strong>
        <em>{{ briefNotice.title }}</em>
      </span>
      <ChevronRight :size="17" aria-hidden="true" />
    </button>

    <section class="home-market-section">
      <div class="section-heading market-heading">
        <h2>{{ t('markets.title') }}</h2>
        <button class="announcement-more" type="button" @click="router.push({ name: 'news' })">
          {{ t('common.more') }} <ChevronRight :size="16" />
        </button>
      </div>
      <div class="market-tabs" role="tablist" :aria-label="t('home.marketSummary')">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.key"
          :class="{ 'is-active': activeTab === tab.key }"
          @click="selectTab(tab.key)"
        >
          {{ tab.label }}
        </button>
      </div>

      <div v-if="marketStore.error" class="market-error">
        <span>{{ t('common.marketLoadFailed') }}</span>
        <button type="button" :disabled="marketStore.loading" @click="marketStore.refresh(true)">{{ t('common.retry') }}</button>
      </div>
      <div class="ticker-heading" aria-hidden="true">
        <span>{{ t('markets.pair') }}</span>
        <span>{{ t('markets.latestPrice') }}</span>
        <span>{{ t('markets.change24h') }}</span>
      </div>
      <div class="ticker-list">
        <button v-for="ticker in visibleTickers.slice(0, 5)" :key="ticker.symbol" class="ticker-row" type="button" @click="openMarket(ticker.symbol)">
          <span class="ticker-row__asset">
            <AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :size="32" />
            <span class="ticker-row__name">
              <b>{{ ticker.base }}<small>/{{ ticker.quote }}</small></b>
              <em>{{ t('markets.volume', { value: formatCompact(ticker.volume) }) }}</em>
            </span>
          </span>
          <span class="ticker-row__price"><b>{{ formatPrice(ticker.lastPrice) }}</b><small>{{ ticker.quote }}</small></span>
          <span class="ticker-row__change" :class="ticker.changePercent >= 0 ? 'is-up' : 'is-down'">{{ formatPercent(ticker.changePercent) }}</span>
        </button>
      </div>
    </section>

    <section class="home-benefits">
      <button type="button" @click="router.push({ name: 'kyc' })">
        <span><Gift :size="19" aria-hidden="true" /></span>
        <div><strong>{{ t('profile.kyc') }}</strong><small>{{ t('profile.loginDescription') }}</small></div>
        <ChevronRight :size="16" aria-hidden="true" />
      </button>
      <button type="button" @click="router.push({ name: 'security' })">
        <span><ShieldCheck :size="19" aria-hidden="true" /></span>
        <div><strong>{{ t('profile.security') }}</strong><small>{{ t('profile.improveSecurity') }}</small></div>
        <ChevronRight :size="16" aria-hidden="true" />
      </button>
    </section>
  </main>
</template>

<style scoped>
.home-page {
  padding-top: 0;
}

.home-header {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: calc(56px + env(safe-area-inset-top));
  padding: env(safe-area-inset-top) 12px 0;
  position: sticky;
  top: 0;
  isolation: isolate;
  z-index: var(--layer-sticky-header);
}

.home-header > .icon-button {
  justify-self: start;
}

.home-header__logo {
  height: 25px;
  justify-self: center;
  max-width: 118px;
  object-fit: contain;
  width: 100%;
}

.home-header__actions {
  display: flex;
  justify-self: end;
}

.market-search {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: 0;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 9px;
  margin-top: 12px;
  min-height: 46px;
  padding: 0 14px;
  text-align: left;
  width: 100%;
}

.market-search span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-glance {
  background: var(--dark-surface);
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--signal-green);
  color: var(--on-dark-surface);
  margin: 12px -16px 0;
  min-height: 218px;
  overflow: hidden;
  padding: 27px 16px 22px;
  position: relative;
}

.asset-glance::after {
  background: color-mix(in srgb, var(--signal-green) 18%, transparent);
  border: 1px solid color-mix(in srgb, var(--on-dark-surface) 18%, transparent);
  border-radius: 50%;
  content: "";
  height: 150px;
  position: absolute;
  right: -64px;
  top: -66px;
  width: 150px;
}

.asset-glance__label {
  align-items: center;
  color: color-mix(in srgb, var(--on-dark-surface) 66%, transparent);
  display: flex;
  font-size: 13px;
  gap: 7px;
  position: relative;
  z-index: 1;
}

.asset-glance__amount {
  align-items: flex-end;
  display: flex;
  gap: 18px;
  justify-content: space-between;
  position: relative;
  z-index: 1;
}

.asset-glance strong {
  color: var(--on-dark-surface);
  display: block;
  font-family: var(--data-font);
  font-size: 38px;
  font-weight: 760;
  letter-spacing: 0;
  line-height: 1.05;
  margin: 18px 0 12px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.asset-glance p {
  color: color-mix(in srgb, var(--on-dark-surface) 72%, transparent);
  font-size: 12px;
  margin: 0;
  position: relative;
  z-index: 1;
}

.asset-glance__signal {
  align-items: flex-end;
  display: flex;
  flex: 0 0 auto;
  gap: 4px;
  height: 38px;
  margin-bottom: 15px;
}

.asset-glance__signal i {
  background: var(--positive);
  display: block;
  opacity: .32;
  width: 4px;
}

.asset-glance__signal i:nth-child(1) { height: 11px; }
.asset-glance__signal i:nth-child(2) { height: 22px; opacity: .55; }
.asset-glance__signal i:nth-child(3) { height: 16px; opacity: .4; }
.asset-glance__signal i:nth-child(4) { height: 34px; opacity: .92; }
.asset-glance__signal i:nth-child(5) { height: 27px; opacity: .68; }

.funding-actions {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 1fr;
  margin: 0 -16px;
  padding: 12px 16px;
}

.funding-actions .button {
  border-radius: 0;
  min-height: 48px;
}

.shortcut-section {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 12px 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin: 0 -16px;
  padding: 18px 16px;
}

.shortcut-section button {
  align-content: center;
  background: transparent;
  color: var(--ink);
  display: grid;
  gap: 7px;
  justify-items: center;
  min-height: 68px;
  min-width: 0;
  padding: 3px;
  text-align: center;
}

.shortcut-section button > span {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  height: 40px;
  justify-content: center;
  width: 40px;
}

.shortcut-section small {
  color: var(--muted-strong);
  font-size: 10px;
  line-height: 1.2;
  max-width: 100%;
  min-height: 24px;
  overflow-wrap: anywhere;
}

.market-pulse {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--accent);
  color: var(--on-accent);
  display: grid;
  gap: 11px;
  grid-template-columns: 40px minmax(0, 1fr) auto;
  margin: 14px 0 0;
  min-height: 78px;
  padding: 13px;
  text-align: left;
  width: 100%;
}

.market-pulse > span {
  align-items: center;
  background: var(--dark-surface);
  border-radius: 50%;
  color: var(--signal-green);
  display: flex;
  height: 40px;
  justify-content: center;
  width: 40px;
}

.market-pulse div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.market-pulse small {
  color: color-mix(in srgb, var(--on-accent) 66%, transparent);
  font-size: 10px;
}

.market-pulse strong {
  color: var(--on-accent);
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-pulse > svg {
  color: color-mix(in srgb, var(--on-accent) 64%, transparent);
}

.home-market-section {
  border-bottom: 8px solid var(--soft);
  margin: 0 -16px;
  padding: 0 16px 12px;
}

.market-heading {
  gap: 10px;
  margin: 22px 0 0;
}

.market-tabs {
  display: flex;
  gap: 2px;
  min-width: 0;
}

.market-tabs button {
  background: transparent;
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-size: 12px;
  min-height: 44px;
  padding: 0 8px;
  white-space: nowrap;
}

.market-tabs .is-active {
  border-color: var(--signal-green);
  color: var(--ink);
  font-weight: 750;
}

.section-heading__action {
  align-items: center;
  display: inline-flex;
  flex: 0 0 auto;
  gap: 2px;
  min-height: 44px;
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
  margin: 8px 0;
  padding: 8px 10px;
}

.market-error button {
  background: transparent;
  color: var(--negative);
  font-weight: 750;
  min-height: 44px;
  padding: 0 8px;
}

.ticker-heading {
  color: var(--muted);
  display: grid;
  font-size: 10px;
  grid-template-columns: minmax(0, 1.15fr) minmax(78px, .82fr) 76px;
  min-height: 34px;
  padding-top: 7px;
}

.ticker-heading span {
  align-self: center;
}

.ticker-heading span:nth-child(n + 2) {
  text-align: right;
}

.ticker-list {
  display: grid;
}

.ticker-row {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1.15fr) minmax(78px, .82fr) 76px;
  min-height: 70px;
  padding: 7px 0;
  text-align: left;
  width: 100%;
}

.ticker-row__asset {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.ticker-row__name,
.ticker-row__price {
  display: grid;
  min-width: 0;
}

.ticker-row b {
  color: var(--ink);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ticker-row__name b small {
  display: inline;
  font-size: 10px;
  font-weight: 600;
  margin: 0;
}

.ticker-row small,
.ticker-row em {
  color: var(--muted);
  font-size: 10px;
  font-style: normal;
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ticker-row__price {
  text-align: right;
}

.ticker-row__change {
  border: 1px solid currentColor;
  font-size: 11px;
  font-weight: 730;
  min-height: 32px;
  padding: 8px 4px;
  text-align: center;
}

.ticker-row__change.is-up {
  background: var(--positive);
  border-color: var(--positive);
  color: var(--on-positive);
}

.ticker-row__change.is-down {
  background: var(--negative);
  border-color: var(--negative);
  color: var(--on-negative);
}

.announcements {
  padding-bottom: 14px;
}

.announcements .section-heading {
  font-size: 19px;
  margin: 24px 0 6px;
}

.announcement-row {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  display: flex;
  font-size: 13px;
  gap: 12px;
  justify-content: space-between;
  min-height: 58px;
  padding: 8px 0;
  text-align: left;
  width: 100%;
}

.announcement-row span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.announcement-more {
  background: transparent;
  color: var(--accent);
  font-size: 12px;
  font-weight: 700;
  margin-top: 7px;
  min-height: 44px;
  padding: 0 4px;
}

@media (max-width: 360px) {
  .asset-glance,
  .funding-actions,
  .shortcut-section,
  .home-market-section {
    margin-left: -12px;
    margin-right: -12px;
  }

  .asset-glance,
  .funding-actions,
  .home-market-section {
    padding-left: 12px;
    padding-right: 12px;
  }

  .shortcut-section {
    padding-left: 8px;
    padding-right: 8px;
  }
}

@media (max-width: 340px) {
  .home-page .page-content {
    padding-left: 12px;
    padding-right: 12px;
  }

  .asset-glance,
  .funding-actions,
  .shortcut-section,
  .home-market-section {
    margin-left: -12px;
    margin-right: -12px;
  }

  .asset-glance,
  .funding-actions,
  .home-market-section {
    padding-left: 12px;
    padding-right: 12px;
  }

  .shortcut-section {
    padding-left: 8px;
    padding-right: 8px;
  }

  .ticker-heading,
  .ticker-row {
    grid-template-columns: minmax(0, 1fr) minmax(72px, .76fr) 72px;
  }

  .ticker-row__asset {
    gap: 6px;
  }

  .ticker-row__name em {
    display: none;
  }

  .market-tabs button {
    padding-inline: 5px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .asset-glance__signal i {
    animation: none;
  }
}
</style>

<style scoped>
.home-page {
  background-color: var(--surface);
  background-image:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px);
  background-size: 48px 48px;
  padding-top: 0;
}

.home-header {
  background: var(--surface);
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: calc(64px + env(safe-area-inset-top));
  padding: env(safe-area-inset-top) 16px 0;
}

.home-header__brand {
  align-items: center;
  align-self: stretch;
  background: transparent;
  display: flex;
  justify-self: start;
  min-width: 116px;
  padding: 0;
  position: relative;
}

.home-header__brand::after {
  background: var(--accent);
  bottom: 3px;
  content: '';
  height: 2px;
  left: 0;
  position: absolute;
  width: 34px;
}

.home-header__logo {
  height: 30px;
  justify-self: start;
  max-width: 116px;
  object-position: left center;
}

.home-header__actions {
  gap: 6px;
}

.home-header__actions .icon-button,
.home-scan {
  background: var(--surface);
  border: 1px solid var(--line);
}

.home-header__notification {
  position: relative;
}

.home-header__notification::after {
  background: var(--accent);
  border: 2px solid var(--surface);
  border-radius: 50%;
  content: '';
  height: 8px;
  position: absolute;
  right: 2px;
  top: 2px;
  width: 8px;
}

.home-workspace {
  padding: 8px 16px 0;
}

.home-utility-row {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) 44px;
  height: 56px;
}

.market-search {
  background: var(--surface);
  border: 1px solid var(--line);
  height: 44px;
  margin: 0;
  min-height: 44px;
  padding: 0 13px;
}

.home-scan {
  border-radius: 50%;
  color: var(--ink);
  display: grid;
  height: 44px;
  min-width: 44px;
  padding: 0;
  place-items: center;
}

.asset-glance {
  background:
    linear-gradient(128deg, color-mix(in srgb, var(--signal-coral) 14%, transparent), transparent 44%),
    linear-gradient(315deg, color-mix(in srgb, var(--positive) 10%, transparent), transparent 46%),
    var(--surface);
  border-bottom: 1px solid var(--line-strong);
  border-top: 1px solid var(--line-strong);
  color: var(--ink);
  margin: 4px -16px 0;
  min-height: 288px;
  padding: 26px 16px 14px;
}

.asset-glance::before {
  color: var(--muted);
  content: 'PORTFOLIO / LIVE CAPITAL';
  font-family: var(--data-font);
  font-size: 8px;
  position: absolute;
  right: 16px;
  top: 11px;
}

.asset-glance::after {
  content: none;
}

.asset-glance__heading {
  align-items: flex-start;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  position: relative;
  z-index: 1;
}

.asset-glance__heading > div:first-child {
  min-width: 0;
}

.asset-glance__label {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 12px;
  gap: 7px;
}

.asset-glance__visibility {
  background: transparent;
  color: var(--ink);
  display: grid;
  height: 44px;
  margin: -13px 0;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
  place-items: center;
}

.asset-glance strong.asset-glance__balance {
  color: var(--ink);
  display: block;
  font-size: 38px;
  font-weight: 680;
  line-height: 1;
  margin: 15px 0 0;
  max-width: 252px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-glance__balance small {
  font-family: inherit;
  font-size: 10px;
  font-weight: 700;
}

.asset-glance__fiat {
  color: var(--muted);
  display: block;
  font-size: 11px;
  margin-top: 9px;
}

.asset-glance__return {
  display: grid;
  flex: 0 0 auto;
  gap: 4px;
  justify-items: end;
  padding-top: 1px;
}

.asset-glance__return span {
  color: var(--muted);
  font-size: 10px;
}

.asset-glance__return strong {
  color: var(--positive);
  font-size: 13px;
  margin: 0;
}

.asset-glance__return small {
  font-size: 10px;
}

.asset-glance__chart {
  border-bottom: 1px solid var(--line);
  height: 86px;
  margin-top: 8px;
  overflow: hidden;
  position: relative;
}

.asset-glance__chart::before,
.asset-glance__chart::after {
  background: var(--line);
  content: '';
  height: 1px;
  left: 0;
  position: absolute;
  right: 0;
  top: 27px;
}

.asset-glance__chart::after {
  top: 58px;
}

.asset-glance__chart-line {
  background: var(--positive);
  clip-path: polygon(0 76%, 12% 72%, 22% 56%, 34% 54%, 44% 66%, 54% 44%, 64% 46%, 73% 31%, 82% 38%, 91% 21%, 100% 8%, 100% 12%, 91% 25%, 82% 42%, 73% 35%, 64% 50%, 54% 48%, 44% 70%, 34% 58%, 22% 60%, 12% 76%, 0 80%);
  inset: 0;
  position: absolute;
}

.asset-glance__chart > i {
  background: var(--positive);
  border-radius: 50%;
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--positive) 16%, transparent);
  height: 7px;
  position: absolute;
  right: 0;
  top: 4px;
  width: 7px;
}

.asset-glance__periods {
  align-items: end;
  display: flex;
  gap: 8px;
  height: 42px;
}

.asset-glance__periods button {
  background: transparent;
  border: 1px solid transparent;
  color: var(--muted);
  font-size: 10px;
  min-height: 40px;
  min-width: 52px;
  padding: 0 6px;
}

.asset-glance__periods .is-active {
  border-color: var(--line-strong);
  color: var(--ink);
  font-weight: 720;
}

:global(:root[data-theme='dark']) .asset-glance {
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--signal-green) 12%, transparent), transparent 45%),
    var(--dark-surface);
  color: var(--on-dark-surface);
}

:global(:root[data-theme='dark']) .asset-glance::before,
:global(:root[data-theme='dark']) .asset-glance__label,
:global(:root[data-theme='dark']) .asset-glance__fiat,
:global(:root[data-theme='dark']) .asset-glance__return span {
  color: color-mix(in srgb, var(--on-dark-surface) 62%, transparent);
}

:global(:root[data-theme='dark']) .asset-glance strong.asset-glance__balance,
:global(:root[data-theme='dark']) .asset-glance__visibility,
:global(:root[data-theme='dark']) .asset-glance__periods .is-active {
  color: var(--on-dark-surface);
}

.funding-actions {
  background: var(--surface);
  gap: 8px;
  margin: 0;
  padding: 12px 16px;
}

.funding-actions button {
  align-items: center;
  border-radius: 0;
  display: flex;
  font-size: 13px;
  font-weight: 750;
  gap: 8px;
  justify-content: center;
  min-height: 50px;
}

.buy-crypto {
  background: var(--signal-green);
  border: 1px solid var(--signal-green);
  color: var(--on-positive);
}

.deposit-crypto {
  background: var(--surface-elevated);
  border: 1px solid var(--line-strong);
  color: var(--ink);
}

.shortcut-section {
  background: color-mix(in srgb, var(--surface) 94%, transparent);
  gap: 16px 8px;
  margin: 0;
  min-height: 188px;
  padding: 16px 16px 12px;
}

.shortcut-section button {
  min-height: 68px;
}

.shortcut-section button > span {
  background: var(--surface);
  height: 40px;
  width: 40px;
}

.shortcut-section small {
  color: var(--ink);
  font-size: 11px;
  min-height: 14px;
}

.announcement-row {
  align-items: center;
  background: var(--signal-coral);
  border: 1px solid var(--signal-coral);
  color: var(--on-accent);
  display: grid;
  gap: 10px;
  grid-template-columns: 38px minmax(0, 1fr) 20px;
  margin: 14px 16px 0;
  min-height: 86px;
  padding: 14px;
  width: calc(100% - 32px);
}

.announcement-row .brief-icon {
  background: var(--dark-surface);
  border-radius: 50%;
  color: var(--on-dark-surface);
  display: grid;
  height: 36px;
  place-items: center;
  width: 36px;
}

.announcement-row > span:nth-child(2) {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.announcement-row small {
  color: color-mix(in srgb, var(--on-accent) 64%, transparent);
  font-size: 10px;
}

.announcement-row strong {
  color: var(--on-accent);
  font-size: 13px;
}

.announcement-row em {
  color: color-mix(in srgb, var(--on-accent) 70%, transparent);
  font-size: 10px;
  font-style: normal;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:global(:root[data-theme='dark']) .announcement-row {
  background: var(--ink);
  border-color: var(--ink);
  color: var(--surface);
}

.home-market-section {
  background: var(--surface);
  margin: 0;
  padding: 22px 16px 10px;
}

.market-heading {
  margin: 0 0 8px;
}

.market-heading h2 {
  font-size: 22px;
  margin: 0;
}

.announcement-more {
  align-items: center;
  color: var(--muted-strong);
  display: inline-flex;
  gap: 2px;
  margin: 0;
  padding: 0 4px;
}

.market-tabs {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 0;
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.market-tabs button {
  font-size: 10px;
  min-width: 0;
  overflow: hidden;
  padding: 0 2px;
  text-overflow: ellipsis;
}

.ticker-heading {
  grid-template-columns: minmax(0, 1fr) minmax(82px, .9fr) 78px;
}

.ticker-row {
  grid-template-columns: minmax(0, 1fr) minmax(82px, .9fr) 78px;
  min-height: 70px;
}

.ticker-row__change.is-up {
  background: var(--positive);
  color: var(--on-positive);
}

.home-benefits {
  background: var(--surface);
  padding: 6px 16px 8px;
}

.home-benefits button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 38px minmax(0, 1fr) 18px;
  min-height: 68px;
  padding: 0;
  text-align: left;
  width: 100%;
}

.home-benefits button > span {
  background: var(--soft);
  border-radius: 5px;
  display: grid;
  height: 36px;
  place-items: center;
  width: 36px;
}

.home-benefits button div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.home-benefits button strong {
  font-size: 12px;
}

.home-benefits button small {
  color: var(--muted);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 360px) {
  .home-header,
  .home-workspace,
  .funding-actions,
  .home-market-section,
  .home-benefits {
    padding-left: 12px;
    padding-right: 12px;
  }

  .asset-glance {
    margin-left: -12px;
    margin-right: -12px;
    padding-left: 12px;
    padding-right: 12px;
  }

  .shortcut-section {
    padding-left: 8px;
    padding-right: 8px;
  }

  .announcement-row {
    margin-left: 12px;
    margin-right: 12px;
    width: calc(100% - 24px);
  }
}

@media (max-width: 340px) {
  .home-header__brand,
  .home-header__logo {
    min-width: 104px;
    width: 104px;
  }

  .asset-glance {
    min-height: 316px;
  }

  .asset-glance__heading {
    display: grid;
  }

  .asset-glance__return {
    grid-auto-flow: column;
    justify-content: start;
  }

  .asset-glance strong.asset-glance__balance {
    font-size: 32px;
  }

  .asset-glance__periods {
    justify-content: space-between;
  }

  .asset-glance__periods button {
    min-width: 44px;
  }

  .ticker-heading,
  .ticker-row {
    grid-template-columns: minmax(0, 1fr) minmax(72px, .76fr) 72px;
  }
}
</style>
