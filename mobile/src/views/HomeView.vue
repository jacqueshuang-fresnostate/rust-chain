<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  Bell,
  CandlestickChart,
  ChevronRight,
  CircleDollarSign,
  Eye,
  Gauge,
  Grid2X2,
  Landmark,
  Layers3,
  Moon,
  Rocket,
  Search,
  Sun,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import { fetchNews } from '@/api/news'
import { fetchMarginWallets } from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { fallbackNews } from '@/data/fallback'
import { formatCompact, formatFiat, formatPercent, formatPrice } from '@/core/format'
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

type HomeTab = 'favorites' | 'popular' | 'gainers'
type TradeMode = 'spot' | 'contract'

const activeTab = ref<HomeTab>('popular')
const announcements = ref<NewsItem[]>(fallbackNews)
const usingFallbackNews = ref(true)
const spotAccounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const assetEstimateReady = ref(false)

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
  const rows = [...marketStore.topTickers]
  if (activeTab.value === 'gainers') return rows.sort((left, right) => right.changePercent - left.changePercent)
  if (activeTab.value === 'popular') return rows.sort((left, right) => right.volume - left.volume)
  return rows
})

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
      <button class="icon-button" type="button" :aria-label="t('home.productCenter')" @click="router.push({ name: 'products' })">
        <Grid2X2 :size="22" />
      </button>
      <img :src="logo" class="home-header__logo" alt="HIPPO" />
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
        <button class="icon-button" type="button" :aria-label="t('home.openMessageCenter')" @click="router.push({ name: 'message-center' })">
          <Bell :size="21" aria-hidden="true" />
        </button>
      </div>
    </header>

    <div class="page-content">
      <button class="market-search" type="button" @click="router.replace({ name: 'markets' })">
        <Search :size="19" />
        <span>{{ t('home.searchPlaceholder') }}</span>
      </button>

      <section class="asset-glance" :aria-label="t('home.assetOverview')">
        <div class="asset-glance__label">
          <span>{{ t('home.totalAssetValue') }}</span>
          <Eye :size="16" />
        </div>
        <div class="asset-glance__amount">
          <strong class="numeric">{{ displayedAssetEstimate }}</strong>
          <span class="asset-glance__signal" aria-hidden="true"><i /><i /><i /><i /><i /></span>
        </div>
        <p>{{ session.isAuthenticated ? t('home.memberAssetHint') : t('home.guestAssetHint') }}</p>
      </section>

      <div class="funding-actions">
        <button class="button button--primary" type="button" @click="router.push({ name: 'deposit-asset' })">
          {{ t('home.deposit') }}
        </button>
        <button class="button button--secondary" type="button" @click="openTrade('spot')">
          {{ t('home.trade') }}
        </button>
      </div>

      <section class="shortcut-section" :aria-label="t('home.productCenter')">
        <button type="button" @click="openTrade('spot')"><span><CandlestickChart :size="20" /></span><small>{{ t('trade.spot') }}</small></button>
        <button type="button" @click="openTrade('contract')"><span><Layers3 :size="20" /></span><small>{{ t('trade.contract') }}</small></button>
        <button type="button" @click="router.push({ name: 'seconds' })"><span><Gauge :size="20" /></span><small>{{ t('seconds.title') }}</small></button>
        <button type="button" @click="router.push({ name: 'earn' })"><span><Landmark :size="20" /></span><small>{{ t('products.earn') }}</small></button>
        <button type="button" @click="router.push({ name: 'loan' })"><span><CircleDollarSign :size="20" /></span><small>{{ t('products.loan') }}</small></button>
        <button type="button" @click="router.push({ name: 'new-coins' })"><span><Rocket :size="20" /></span><small>{{ t('products.newCoins') }}</small></button>
        <button type="button" @click="router.push({ name: 'prediction' })"><span><Activity :size="20" /></span><small>{{ t('products.prediction') }}</small></button>
        <button type="button" @click="router.push({ name: 'products' })"><span><Grid2X2 :size="20" /></span><small>{{ t('home.productCenter') }}</small></button>
      </section>

      <button class="market-pulse" type="button" :aria-label="t('home.marketSummary')" @click="router.replace({ name: 'markets' })">
        <span><Activity :size="20" /></span>
        <div>
          <small>{{ t('home.marketUpdates') }}</small>
          <strong>{{ marketStore.error ? t('common.marketUnavailable') : t('common.liveData') }}</strong>
        </div>
        <ChevronRight :size="18" />
      </button>

      <section class="home-market-section">
        <div class="section-heading market-heading">
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
          <button class="section-heading__action" type="button" @click="router.replace({ name: 'markets' })">
            {{ t('common.more') }} <ChevronRight :size="16" />
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
.home-page {
  --home-contrast-ink: var(--surface);
  padding-top: 0;
}

:global(:root[data-theme='dark']) .home-page {
  --home-contrast-ink: var(--ink);
}

.home-header {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: minmax(88px, 1fr) minmax(96px, auto) minmax(88px, 1fr);
  min-height: calc(56px + env(safe-area-inset-top));
  padding: env(safe-area-inset-top) 12px 0;
  position: sticky;
  top: 0;
  z-index: 70;
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
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
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
  border-top: 3px solid var(--accent);
  color: var(--home-contrast-ink);
  margin: 14px -20px 0;
  min-height: 204px;
  overflow: hidden;
  padding: 25px 20px 22px;
  position: relative;
}

.asset-glance::after {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  border: 1px solid color-mix(in srgb, var(--home-contrast-ink) 18%, transparent);
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
  color: color-mix(in srgb, var(--home-contrast-ink) 66%, transparent);
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
  color: var(--home-contrast-ink);
  display: block;
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
  color: color-mix(in srgb, var(--home-contrast-ink) 72%, transparent);
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
  margin: 0 -20px;
  padding: 12px 20px;
}

.funding-actions .button {
  border-radius: var(--radius);
  min-height: 48px;
}

.shortcut-section {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 12px 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin: 0 -20px;
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
  color: var(--accent);
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
  background: var(--ink);
  border: 1px solid var(--ink);
  color: var(--home-contrast-ink);
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
  background: var(--positive);
  border-radius: 50%;
  color: var(--on-positive);
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
  color: color-mix(in srgb, var(--home-contrast-ink) 64%, transparent);
  font-size: 10px;
}

.market-pulse strong {
  color: var(--home-contrast-ink);
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-pulse > svg {
  color: color-mix(in srgb, var(--home-contrast-ink) 55%, transparent);
}

.home-market-section {
  border-bottom: 8px solid var(--soft);
  margin: 0 -20px;
  padding: 0 20px 12px;
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
  border-color: var(--ink);
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
    margin-left: -16px;
    margin-right: -16px;
  }

  .asset-glance,
  .funding-actions,
  .home-market-section {
    padding-left: 16px;
    padding-right: 16px;
  }

  .shortcut-section {
    padding-left: 12px;
    padding-right: 12px;
  }
}

@media (max-width: 340px) {
  .home-page .page-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .asset-glance,
  .funding-actions,
  .shortcut-section,
  .home-market-section {
    margin-left: -14px;
    margin-right: -14px;
  }

  .asset-glance,
  .funding-actions,
  .home-market-section {
    padding-left: 14px;
    padding-right: 14px;
  }

  .shortcut-section {
    padding-left: 10px;
    padding-right: 10px;
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
