<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import { useRoute, useRouter } from 'vue-router'
import { Icon } from '@iconify/vue'
import {
  createPredictionOrder,
  createPredictionQuote,
  fetchPredictionConfig,
  fetchPredictionMarket,
  fetchPredictionMarkets,
  type PredictionMarket,
  type PredictionOutcome,
  type PredictionQuote,
  type PredictionStakeAsset,
} from '@/api/prediction'
import { getWallets, type MemberWallet } from '@/api/asset'
import PairLogo from '@/components/common/PairLogo.vue'
import { readAuthToken } from '@/utils/authStorage'
import { formatNumber } from '@/utils/format'
import { localizePredictionMarketText } from '@/utils/predictionLocale'

type BrowseKey = 'all' | 'trending' | 'popular' | 'volume' | 'ending' | 'competitive'
type LocalizedPredictionMarket = PredictionMarket & {
  localizedTitle: string
  localizedDescription: string
  localizedCategory: string
  localizedYesLabel: string
  localizedNoLabel: string
  searchIndex: string
}
type StakeAssetOption = PredictionStakeAsset & {
  balance: number
  logoUrl?: string
}

const { t, locale } = useI18n()
const toast = useToast()
const route = useRoute()
const router = useRouter()

const markets = ref<PredictionMarket[]>([])
const marketDetail = ref<PredictionMarket | null>(null)
const allowedAssets = ref<PredictionStakeAsset[]>([])
const walletAccounts = ref<MemberWallet[]>([])
const selectedMarketId = ref<number | null>(null)
const outcome = ref<PredictionOutcome>('yes')
const assetId = ref('')
const stakeAmount = ref('')
const quote = ref<PredictionQuote | null>(null)
const loading = ref(false)
const detailLoading = ref(false)
const quoteLoading = ref(false)
const orderLoading = ref(false)
const detailError = ref('')
const searchText = ref('')
const assetSearchText = ref('')
const assetSelectorOpen = ref(false)
const activeBrowse = ref<BrowseKey>('all')
const activeTopic = ref('all')

const quickStakeAmounts = ['25', '50', '100']
const browseFilters: Array<{ key: BrowseKey; labelKey: string; icon: string }> = [
  { key: 'all', labelKey: 'prediction.browse_all', icon: 'lucide:layout-grid' },
  { key: 'trending', labelKey: 'prediction.browse_trending', icon: 'lucide:flame' },
  { key: 'popular', labelKey: 'prediction.browse_popular', icon: 'lucide:sparkles' },
  { key: 'volume', labelKey: 'prediction.browse_volume', icon: 'lucide:bar-chart-3' },
  { key: 'ending', labelKey: 'prediction.browse_ending', icon: 'lucide:timer' },
  { key: 'competitive', labelKey: 'prediction.browse_competitive', icon: 'lucide:scale' },
]

const routeMarketId = computed(() => {
  const raw = Array.isArray(route.params.id) ? route.params.id[0] : route.params.id
  const id = Number.parseInt(String(raw || ''), 10)
  return Number.isFinite(id) && id > 0 ? id : null
})

const isDetailView = computed(() => Boolean(routeMarketId.value))

const localizedMarkets = computed<LocalizedPredictionMarket[]>(() => markets.value.map(localizeMarket))
const localizedMarketDetail = computed(() => marketDetail.value ? localizeMarket(marketDetail.value) : null)

const selectedMarket = computed(() => {
  if (isDetailView.value) {
    return localizedMarketDetail.value
      || localizedMarkets.value.find((market) => market.id === routeMarketId.value)
      || null
  }
  return selectedMarketId.value
    ? localizedMarkets.value.find((market) => market.id === selectedMarketId.value) || null
    : null
})

const selectedPrice = computed(() => {
  const market = selectedMarket.value
  if (!market) return '0'
  return outcome.value === 'yes' ? market.yes_price : market.no_price
})
const selectedPriceNumber = computed(() => numberValue(selectedPrice.value))

const selectedOutcomeLabel = computed(() => {
  const market = selectedMarket.value
  if (!market) return '--'
  return outcome.value === 'yes' ? market.localizedYesLabel : market.localizedNoLabel
})

const effectiveAssets = computed(() => {
  const market = selectedMarket.value
  const override = market?.allowed_asset_ids_override_json
  if (Array.isArray(override) && override.length > 0) {
    const ids = new Set(override.map(String))
    return allowedAssets.value.filter(asset => ids.has(String(asset.asset_id)))
  }
  return allowedAssets.value
})

const canQuote = computed(() => Boolean(selectedMarket.value && assetId.value && Number(stakeAmount.value) > 0 && effectiveAssets.value.length > 0))
const hasAuthSession = computed(() => Boolean(readAuthToken()))

const stakeAssetOptions = computed<StakeAssetOption[]>(() => effectiveAssets.value.map((asset) => {
  const wallet = findWalletForAsset(asset)
  return {
    ...asset,
    balance: wallet?.balance ?? 0,
    logoUrl: wallet?.coin.logoUrl,
  }
}))

const selectedStakeAsset = computed(() => stakeAssetOptions.value.find((asset) => String(asset.asset_id) === assetId.value) ?? stakeAssetOptions.value[0] ?? null)

const filteredStakeAssetOptions = computed(() => {
  const keyword = assetSearchText.value.trim().toUpperCase()
  if (!keyword) return stakeAssetOptions.value
  return stakeAssetOptions.value.filter((asset) => asset.asset_symbol.toUpperCase().includes(keyword))
})

const selectedAssetBalanceText = computed(() => {
  const asset = selectedStakeAsset.value
  if (!asset) return '--'
  return `${formatAmount(asset.balance)} ${asset.asset_symbol}`
})

const effectivePayoutCapAmount = computed(() => {
  const asset = selectedStakeAsset.value
  const market = selectedMarket.value
  if (!asset) return 0
  const overrideCap = optionalNumberValue(market?.payout_cap_overrides_json?.[String(asset.asset_id)])
  return overrideCap === null ? numberValue(asset.max_payout_amount) : overrideCap
})

const maxStakeByPayoutCap = computed(() => {
  const cap = effectivePayoutCapAmount.value
  const price = selectedPriceNumber.value
  if (cap <= 0 || price <= 0) return Number.POSITIVE_INFINITY
  return cap * price
})

const maxStakeAmount = computed(() => {
  const asset = selectedStakeAsset.value
  if (!asset) return 0
  const cappedMax = maxStakeByPayoutCap.value
  if (!Number.isFinite(cappedMax)) return asset.balance
  return Math.max(0, Math.min(asset.balance, cappedMax))
})

const maxStakeAmountText = computed(() => {
  const asset = selectedStakeAsset.value
  if (!asset) return '--'
  return `${formatAmount(maxStakeAmount.value)} ${asset.asset_symbol}`
})

const payoutCapText = computed(() => {
  const asset = selectedStakeAsset.value
  if (!asset) return '--'
  return formatPayoutCapAmount(effectivePayoutCapAmount.value, asset.asset_symbol)
})

const stakeAmountError = computed(() => {
  const rawAmount = stakeAmount.value.trim()
  if (!rawAmount) return ''
  const amount = Number(rawAmount)
  const asset = selectedStakeAsset.value
  if (!Number.isFinite(amount) || amount <= 0) return t('prediction.amount_must_be_positive')
  if (!asset) return ''
  if (hasAuthSession.value && amount > asset.balance) {
    return t('prediction.amount_exceeds_balance', { amount: selectedAssetBalanceText.value })
  }
  const cappedMax = maxStakeByPayoutCap.value
  if (Number.isFinite(cappedMax) && amount > cappedMax) {
    return t('prediction.amount_exceeds_payout_cap', {
      amount: `${formatAmount(cappedMax)} ${asset.asset_symbol}`,
    })
  }
  return ''
})

const topicFilters = computed(() => {
  const categories = Array.from(new Set(localizedMarkets.value.map((market) => market.localizedCategory).filter(Boolean))).sort()
  return [
    { key: 'all', label: t('prediction.all_categories') },
    ...categories.map((category) => ({ key: category, label: category })),
  ]
})

const featuredMarkets = computed(() => sortMarkets(localizedMarkets.value, 'popular').slice(0, 4))

const filteredMarkets = computed(() => {
  const keyword = searchText.value.trim().toLowerCase()
  const filtered = localizedMarkets.value.filter((market) => {
    if (activeTopic.value !== 'all' && market.localizedCategory !== activeTopic.value) return false
    if (keyword && !market.searchIndex.includes(keyword)) return false
    if (activeBrowse.value === 'ending') return isEndingSoon(market)
    if (activeBrowse.value === 'competitive') return Math.abs(numberValue(market.yes_price) - 0.5) <= 0.15
    return true
  })
  return sortMarkets(filtered, activeBrowse.value)
})

const relatedMarkets = computed(() => {
  const market = selectedMarket.value
  if (!market) return []
  return sortMarkets(
    localizedMarkets.value.filter(item => item.id !== market.id && item.localizedCategory === market.localizedCategory),
    'popular',
  ).slice(0, 4)
})

const summaryCards = computed(() => [
  {
    key: 'active',
    label: t('prediction.active_markets'),
    value: String(localizedMarkets.value.length),
    icon: 'mdi:chart-box-outline',
  },
  {
    key: 'assets',
    label: t('prediction.supported_assets'),
    value: String(allowedAssets.value.length),
    icon: 'mdi:wallet-outline',
  },
  {
    key: 'volume',
    label: t('prediction.total_volume'),
    value: formatCompactAmount(totalVolume.value),
    icon: 'mdi:chart-line',
  },
  {
    key: 'ending',
    label: t('prediction.ending_soon'),
    value: String(endingSoonCount.value),
    icon: 'mdi:timer-sand',
  },
])

const totalVolume = computed(() => localizedMarkets.value.reduce((sum, market) => sum + numberValue(market.volume), 0))
const endingSoonCount = computed(() => localizedMarkets.value.filter(isEndingSoon).length)
const quoteExpiresText = computed(() => quote.value?.expires_at ? formatDateTime(quote.value.expires_at) : '--')

watch([selectedMarketId, outcome, assetId, stakeAmount], () => {
  quote.value = null
})

watch(effectiveAssets, (assets) => {
  if (!assets.some(asset => String(asset.asset_id) === assetId.value)) {
    assetId.value = assets[0] ? String(assets[0].asset_id) : ''
  }
}, { immediate: true })

watch(topicFilters, (filters) => {
  if (!filters.some((filter) => filter.key === activeTopic.value)) {
    activeTopic.value = 'all'
  }
})

watch(routeMarketId, (id) => {
  if (!id) {
    marketDetail.value = null
    detailError.value = ''
    selectedMarketId.value = null
    return
  }
  selectedMarketId.value = id
  void loadMarketDetail(id)
}, { immediate: true })

watch(() => route.query.outcome, (value) => {
  const next = Array.isArray(value) ? value[0] : value
  if (next === 'yes' || next === 'no') {
    outcome.value = next
  }
}, { immediate: true })

onMounted(loadPage)

function localizeMarket(market: PredictionMarket): LocalizedPredictionMarket {
  const currentLocale = String(locale.value || '')
  const localizedTitle = localizePredictionMarketText(market.title, currentLocale, 'title', market.title_i18n_json)
  const localizedDescription = localizePredictionMarketText(market.description || '', currentLocale, 'description', market.description_i18n_json)
  const localizedCategory = localizePredictionMarketText(market.category || t('prediction.general'), currentLocale, 'category', market.category_i18n_json)
  const localizedYesLabel = localizePredictionMarketText(market.outcome_yes_label || 'YES', currentLocale, 'outcome', market.outcome_yes_label_i18n_json)
  const localizedNoLabel = localizePredictionMarketText(market.outcome_no_label || 'NO', currentLocale, 'outcome', market.outcome_no_label_i18n_json)
  return {
    ...market,
    localizedTitle,
    localizedDescription,
    localizedCategory,
    localizedYesLabel,
    localizedNoLabel,
    searchIndex: [
      localizedTitle,
      localizedDescription,
      localizedCategory,
      market.title,
      market.description,
      market.category,
    ].filter(Boolean).join(' ').toLowerCase(),
  }
}

async function loadPage() {
  loading.value = true
  try {
    const [configResponse, marketResponse] = await Promise.all([
      fetchPredictionConfig(),
      fetchPredictionMarkets(150),
    ])
    allowedAssets.value = configResponse.data.allowed_assets
    markets.value = marketResponse.data
    await loadWalletBalances()
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.load_failed')))
  } finally {
    loading.value = false
  }
}

async function loadWalletBalances() {
  if (!hasAuthSession.value) {
    walletAccounts.value = []
    return
  }
  const walletResponse = await getWallets()
  walletAccounts.value = walletResponse.data.data
}

async function loadMarketDetail(id: number) {
  detailLoading.value = true
  detailError.value = ''
  try {
    const response = await fetchPredictionMarket(id)
    marketDetail.value = response.data
    selectedMarketId.value = response.data.id
  } catch (error) {
    marketDetail.value = null
    detailError.value = errorMessage(error, t('prediction.detail_load_failed'))
  } finally {
    detailLoading.value = false
  }
}

async function refreshPage() {
  await loadPage()
  if (routeMarketId.value) {
    await loadMarketDetail(routeMarketId.value)
  }
}

function openMarket(market: LocalizedPredictionMarket, nextOutcome?: PredictionOutcome) {
  selectedMarketId.value = market.id
  if (nextOutcome) outcome.value = nextOutcome
  void router.push({
    name: 'PredictionDetail',
    params: { id: String(market.id) },
    query: nextOutcome ? { outcome: nextOutcome } : undefined,
  })
}

function backToMarkets() {
  void router.push({ name: 'Prediction' })
}

function clearFilters() {
  searchText.value = ''
  activeBrowse.value = 'all'
  activeTopic.value = 'all'
}

async function requestQuote() {
  if (!canQuote.value || !selectedMarket.value) return
  if (stakeAmountError.value) {
    toast.error(stakeAmountError.value)
    return
  }
  if (!hasAuthSession.value) {
    toast.error(t('prediction.login_required'))
    await router.push({ name: 'Login', query: { redirect: route.fullPath } })
    return
  }
  quoteLoading.value = true
  try {
    const response = await createPredictionQuote({
      market_id: selectedMarket.value.id,
      outcome: outcome.value,
      asset_id: Number(assetId.value),
      stake_amount: stakeAmount.value.trim(),
    })
    quote.value = response.data
    toast.success(t('prediction.quote_ready'))
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.quote_failed')))
  } finally {
    quoteLoading.value = false
  }
}

async function submitOrder() {
  if (!quote.value) return
  if (!hasAuthSession.value) {
    toast.error(t('prediction.login_required'))
    await router.push({ name: 'Login', query: { redirect: route.fullPath } })
    return
  }
  orderLoading.value = true
  try {
    await createPredictionOrder({
      quote_id: quote.value.quote_id,
      idempotency_key: `pc-prediction-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
    })
    toast.success(t('prediction.order_success'))
    quote.value = null
    stakeAmount.value = ''
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.order_failed')))
  } finally {
    orderLoading.value = false
  }
}

function selectStakeAsset(asset: StakeAssetOption) {
  assetId.value = String(asset.asset_id)
  assetSearchText.value = ''
  assetSelectorOpen.value = false
}

function setMaxStakeAmount() {
  const asset = selectedStakeAsset.value
  if (!asset) return
  stakeAmount.value = toStakeInputAmount(maxStakeAmount.value)
}

function sortMarkets(items: LocalizedPredictionMarket[], sort: BrowseKey) {
  const marketsToSort = [...items]
  if (sort === 'volume') {
    return marketsToSort.sort((a, b) => numberValue(b.volume) - numberValue(a.volume))
  }
  if (sort === 'ending') {
    return marketsToSort.sort((a, b) => dateSortValue(a.end_at) - dateSortValue(b.end_at))
  }
  if (sort === 'competitive') {
    return marketsToSort.sort((a, b) => Math.abs(numberValue(a.yes_price) - 0.5) - Math.abs(numberValue(b.yes_price) - 0.5))
  }
  return marketsToSort.sort((a, b) => (numberValue(b.volume) + numberValue(b.liquidity)) - (numberValue(a.volume) + numberValue(a.liquidity)))
}

function percentText(value: string | number) {
  const number = Number(value)
  if (!Number.isFinite(number)) return '--'
  return formatNumber(number * 100, 'percent')
}

function probabilityWidth(value: string | number) {
  const number = Number(value)
  if (!Number.isFinite(number)) return '0%'
  return `${Math.min(Math.max(number * 100, 3), 100)}%`
}

function formatAmount(value: string | number) {
  return formatNumber(value, 'amount')
}

function formatPayoutCapAmount(value: string | number | null | undefined, symbol: string) {
  const cap = numberValue(value)
  if (cap <= 0) return t('prediction.unlimited')
  return `${formatAmount(cap)} ${symbol}`
}

function formatCompactAmount(value?: string | number | null) {
  const number = numberValue(value)
  if (number <= 0) return '--'
  return formatNumber(number, 'volume')
}

function formatDateTime(value?: number | null) {
  if (!value) return '--'
  const currentLocale = String(locale.value || '')
  return new Date(value).toLocaleString(currentLocale || undefined)
}

function numberValue(value?: string | number | null) {
  const number = Number(value)
  return Number.isFinite(number) ? number : 0
}

function optionalNumberValue(value?: string | number | null) {
  if (value === undefined || value === null || value === '') return null
  const number = Number(value)
  return Number.isFinite(number) ? number : null
}

function toStakeInputAmount(value: number) {
  if (!Number.isFinite(value) || value <= 0) return ''
  return value.toFixed(8).replace(/\.?0+$/, '')
}

function dateSortValue(value?: number | null) {
  const number = Number(value || 0)
  return Number.isFinite(number) && number > 0 ? number : Number.MAX_SAFE_INTEGER
}

function findWalletForAsset(asset: PredictionStakeAsset) {
  const symbol = asset.asset_symbol.trim().toUpperCase()
  return walletAccounts.value.find((wallet) => String(wallet.id) === String(asset.asset_id) || wallet.coin.coinGroup.toUpperCase() === symbol)
}

function isEndingSoon(market: PredictionMarket) {
  const endAt = Number(market.end_at || 0)
  if (!Number.isFinite(endAt) || endAt <= 0) return false
  const now = Date.now()
  const sevenDays = 7 * 24 * 60 * 60 * 1000
  return endAt > now && endAt - now <= sevenDays
}

function settlementBadgeClass(status: string) {
  if (status === 'open') return 'bg-emerald-500/15 text-emerald-300'
  if (status === 'pending_confirmation') return 'bg-amber-500/15 text-amber-300'
  if (status === 'settled') return 'bg-blue-500/15 text-blue-300'
  return 'bg-muted text-muted-foreground'
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>

<template>
  <div class="h-full overflow-y-auto bg-background text-foreground" @click="assetSelectorOpen = false">
    <div class="mx-auto flex w-full max-w-7xl flex-col gap-5 px-4 py-5 lg:px-6">
      <section class="border-b border-border pb-5">
        <div class="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div class="min-w-0">
            <div class="mb-3 inline-flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1 text-xs font-bold text-primary">
              <Icon icon="mdi:chart-timeline-variant-shimmer" class="h-4 w-4" />
              {{ t('prediction.badge') }}
            </div>
            <h1 class="text-3xl font-black leading-tight md:text-4xl">{{ t('prediction.title') }}</h1>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{{ t('prediction.subtitle') }}</p>
          </div>

          <div class="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:min-w-[520px]">
            <div v-for="item in summaryCards" :key="item.key" class="rounded-lg border border-border bg-card/70 p-3">
              <div class="mb-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
                <span>{{ item.label }}</span>
                <Icon :icon="item.icon" class="h-4 w-4 text-primary" />
              </div>
              <div class="truncate font-mono text-lg font-black">{{ item.value }}</div>
            </div>
          </div>
        </div>

        <div class="mt-5 flex gap-2 overflow-x-auto pb-1">
          <button
            v-for="filter in browseFilters"
            :key="filter.key"
            type="button"
            class="inline-flex h-10 shrink-0 items-center gap-2 rounded-full border px-4 text-sm font-bold transition-colors"
            :class="activeBrowse === filter.key ? 'border-primary bg-primary text-primary-foreground' : 'border-border bg-card text-muted-foreground hover:text-foreground'"
            @click="activeBrowse = filter.key"
          >
            <Icon :icon="filter.icon" class="h-4 w-4" />
            {{ t(filter.labelKey) }}
          </button>
        </div>

        <div class="mt-3 flex gap-2 overflow-x-auto pb-1">
          <button
            v-for="topic in topicFilters"
            :key="topic.key"
            type="button"
            class="shrink-0 rounded-full px-3 py-1.5 text-xs font-bold transition-colors"
            :class="activeTopic === topic.key ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/70 hover:text-foreground'"
            @click="activeTopic = topic.key"
          >
            {{ topic.label }}
          </button>
        </div>
      </section>

      <section v-if="!isDetailView" class="flex flex-col gap-5">
        <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 class="text-xl font-black">{{ t('prediction.featured_markets') }}</h2>
            <p class="text-sm text-muted-foreground">{{ t('prediction.market_count', { count: filteredMarkets.length }) }}</p>
          </div>

          <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
            <div class="relative sm:w-80">
              <Icon icon="lucide:search" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <input
                v-model="searchText"
                type="search"
                :placeholder="t('prediction.search_placeholder')"
                class="h-10 w-full rounded-lg border border-input bg-card/70 pl-9 pr-3 text-sm outline-none transition-colors focus:border-primary"
              />
            </div>
            <button
              type="button"
              class="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-border px-3 text-sm font-bold text-muted-foreground transition-colors hover:border-primary hover:text-primary disabled:opacity-50"
              :disabled="loading"
              @click="refreshPage"
            >
              <Icon icon="mdi:refresh" class="h-4 w-4" :class="{ 'animate-spin': loading }" />
              {{ t('prediction.refresh') }}
            </button>
          </div>
        </div>

        <div v-if="loading" class="flex items-center justify-center gap-2 rounded-xl border border-border bg-card/70 px-4 py-20 text-sm text-muted-foreground">
          <Icon icon="mdi:loading" class="h-5 w-5 animate-spin text-primary" />
          {{ t('common.loading') }}
        </div>

        <div v-else-if="filteredMarkets.length === 0" class="rounded-xl border border-dashed border-border bg-card/60 p-10 text-center">
          <Icon icon="mdi:archive-search-outline" class="mx-auto mb-3 h-8 w-8 text-muted-foreground" />
          <div class="font-bold">{{ t('prediction.no_markets') }}</div>
          <button
            v-if="searchText || activeTopic !== 'all' || activeBrowse !== 'all'"
            type="button"
            class="mt-3 text-sm font-bold text-primary hover:opacity-80"
            @click="clearFilters"
          >
            {{ t('prediction.clear_filters') }}
          </button>
        </div>

        <div v-else class="grid gap-4 lg:grid-cols-3">
          <article
            v-for="market in filteredMarkets"
            :key="market.id"
            class="group flex min-h-[290px] flex-col rounded-xl border border-border bg-card/75 p-4 transition hover:border-primary/50 hover:bg-card"
          >
            <button type="button" class="min-w-0 text-left" @click="openMarket(market)">
              <div class="flex items-start gap-3">
                <div class="h-14 w-14 shrink-0 overflow-hidden rounded-lg border border-border bg-background/60">
                  <img v-if="market.image_url" :src="market.image_url" :alt="market.localizedTitle" loading="lazy" class="h-full w-full object-cover" />
                  <div v-else class="flex h-full w-full items-center justify-center text-lg font-black text-primary">
                    {{ market.localizedTitle.slice(0, 1) || '?' }}
                  </div>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="mb-2 flex flex-wrap items-center gap-2">
                    <span class="rounded-md bg-muted px-2 py-1 text-xs font-bold text-muted-foreground">{{ market.localizedCategory }}</span>
                    <span class="rounded-md px-2 py-1 text-xs font-bold" :class="settlementBadgeClass(market.settlement_status)">
                      {{ t(`prediction.settlement_${market.settlement_status}`) }}
                    </span>
                  </div>
                  <h3 class="line-clamp-3 text-base font-black leading-snug group-hover:text-primary">
                    {{ market.localizedTitle }}
                  </h3>
                </div>
              </div>
            </button>

            <p v-if="market.localizedDescription" class="mt-3 line-clamp-2 text-sm text-muted-foreground">
              {{ market.localizedDescription }}
            </p>

            <div class="mt-auto pt-4">
              <div class="mb-3 grid grid-cols-2 gap-2">
                <button
                  type="button"
                  class="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-left transition hover:border-emerald-300"
                  @click.stop="openMarket(market, 'yes')"
                >
                  <div class="truncate text-xs font-bold text-emerald-300">{{ market.localizedYesLabel }}</div>
                  <div class="mt-1 font-mono text-xl font-black text-emerald-200">{{ percentText(market.yes_price) }}</div>
                </button>
                <button
                  type="button"
                  class="rounded-lg border border-rose-500/30 bg-rose-500/10 p-3 text-left transition hover:border-rose-300"
                  @click.stop="openMarket(market, 'no')"
                >
                  <div class="truncate text-xs font-bold text-rose-300">{{ market.localizedNoLabel }}</div>
                  <div class="mt-1 font-mono text-xl font-black text-rose-200">{{ percentText(market.no_price) }}</div>
                </button>
              </div>
              <div class="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
                <span>{{ t('prediction.volume') }} {{ formatCompactAmount(market.volume) }}</span>
                <span v-if="market.end_at">{{ t('prediction.ends') }} {{ formatDateTime(market.end_at) }}</span>
              </div>
            </div>
          </article>
        </div>

        <section v-if="featuredMarkets.length > 0" class="rounded-xl border border-border bg-card/60 p-4">
          <div class="mb-3 flex items-center justify-between gap-3">
            <div>
              <h2 class="text-lg font-black">{{ t('prediction.trending_now') }}</h2>
              <p class="text-sm text-muted-foreground">{{ t('prediction.trending_desc') }}</p>
            </div>
          </div>
          <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <button
              v-for="market in featuredMarkets"
              :key="market.id"
              type="button"
              class="rounded-lg border border-border bg-background/40 p-3 text-left transition hover:border-primary/50"
              @click="openMarket(market)"
            >
              <div class="mb-2 text-xs font-bold text-primary">{{ market.localizedCategory }}</div>
              <div class="line-clamp-2 text-sm font-bold">{{ market.localizedTitle }}</div>
              <div class="mt-3 flex items-center justify-between gap-3 text-xs text-muted-foreground">
                <span>{{ percentText(market.yes_price) }}</span>
                <span>{{ formatCompactAmount(market.volume) }}</span>
              </div>
            </button>
          </div>
        </section>
      </section>

      <section v-else class="flex flex-col gap-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <button type="button" class="inline-flex items-center gap-2 text-sm font-bold text-muted-foreground hover:text-primary" @click="backToMarkets">
            <Icon icon="lucide:arrow-left" class="h-4 w-4" />
            {{ t('prediction.back_to_markets') }}
          </button>
          <button
            type="button"
            class="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-border px-3 text-sm font-bold text-muted-foreground transition-colors hover:border-primary hover:text-primary disabled:opacity-50"
            :disabled="loading || detailLoading"
            @click="refreshPage"
          >
            <Icon icon="mdi:refresh" class="h-4 w-4" :class="{ 'animate-spin': loading || detailLoading }" />
            {{ t('prediction.refresh') }}
          </button>
        </div>

        <div v-if="detailLoading && !selectedMarket" class="flex items-center justify-center gap-2 rounded-xl border border-border bg-card/70 px-4 py-20 text-sm text-muted-foreground">
          <Icon icon="mdi:loading" class="h-5 w-5 animate-spin text-primary" />
          {{ t('common.loading') }}
        </div>

        <div v-else-if="!selectedMarket" class="rounded-xl border border-dashed border-border bg-card/60 p-10 text-center">
          <Icon icon="mdi:alert-circle-outline" class="mx-auto mb-3 h-8 w-8 text-muted-foreground" />
          <div class="font-bold">{{ t('prediction.detail_not_found') }}</div>
          <p class="mt-2 text-sm text-muted-foreground">{{ detailError || t('prediction.detail_not_found_desc') }}</p>
          <button type="button" class="mt-4 rounded-lg bg-primary px-4 py-2 text-sm font-black text-primary-foreground" @click="backToMarkets">
            {{ t('prediction.back_to_markets') }}
          </button>
        </div>

        <div v-else class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(360px,420px)] xl:items-start">
          <main class="min-w-0 space-y-5">
            <section class="rounded-xl border border-border bg-card/75 p-5">
              <div class="flex flex-col gap-5 lg:flex-row">
                <div class="h-44 w-full overflow-hidden rounded-xl border border-border bg-background/60 lg:w-72 lg:shrink-0">
                  <img v-if="selectedMarket.image_url" :src="selectedMarket.image_url" :alt="selectedMarket.localizedTitle" loading="lazy" class="h-full w-full object-cover" />
                  <div v-else class="flex h-full w-full items-center justify-center text-5xl font-black text-primary">
                    {{ selectedMarket.localizedTitle.slice(0, 1) || '?' }}
                  </div>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="mb-3 flex flex-wrap items-center gap-2">
                    <span class="rounded-md bg-muted px-2 py-1 text-xs font-bold text-muted-foreground">{{ selectedMarket.localizedCategory }}</span>
                    <span class="rounded-md px-2 py-1 text-xs font-bold" :class="settlementBadgeClass(selectedMarket.settlement_status)">
                      {{ t(`prediction.settlement_${selectedMarket.settlement_status}`) }}
                    </span>
                    <span v-if="selectedMarket.end_at" class="inline-flex items-center gap-1 rounded-md bg-muted/60 px-2 py-1 text-xs text-muted-foreground">
                      <Icon icon="mdi:clock-outline" class="h-3.5 w-3.5" />
                      {{ formatDateTime(selectedMarket.end_at) }}
                    </span>
                  </div>
                  <h2 class="text-2xl font-black leading-tight md:text-3xl">{{ selectedMarket.localizedTitle }}</h2>
                  <p v-if="selectedMarket.localizedDescription" class="mt-3 text-sm leading-6 text-muted-foreground">
                    {{ selectedMarket.localizedDescription }}
                  </p>
                </div>
              </div>
            </section>

            <section class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <div class="rounded-lg border border-border bg-card/70 p-4">
                <div class="text-xs font-bold text-muted-foreground">{{ t('prediction.volume') }}</div>
                <div class="mt-2 font-mono text-xl font-black">{{ formatCompactAmount(selectedMarket.volume) }}</div>
              </div>
              <div class="rounded-lg border border-border bg-card/70 p-4">
                <div class="text-xs font-bold text-muted-foreground">{{ t('prediction.liquidity') }}</div>
                <div class="mt-2 font-mono text-xl font-black">{{ formatCompactAmount(selectedMarket.liquidity) }}</div>
              </div>
              <div class="rounded-lg border border-border bg-card/70 p-4">
                <div class="text-xs font-bold text-muted-foreground">{{ t('prediction.ends') }}</div>
                <div class="mt-2 text-sm font-bold">{{ formatDateTime(selectedMarket.end_at) }}</div>
              </div>
              <div class="rounded-lg border border-border bg-card/70 p-4">
                <div class="text-xs font-bold text-muted-foreground">{{ t('prediction.last_synced') }}</div>
                <div class="mt-2 text-sm font-bold">{{ formatDateTime(selectedMarket.last_synced_at) }}</div>
              </div>
            </section>

            <section class="rounded-xl border border-border bg-card/75 p-5">
              <div class="mb-4 flex items-center justify-between gap-3">
                <div>
                  <h3 class="text-lg font-black">{{ t('prediction.probability') }}</h3>
                  <p class="text-sm text-muted-foreground">{{ t('prediction.probability_desc') }}</p>
                </div>
              </div>
              <div class="grid gap-3 md:grid-cols-2">
                <button
                  type="button"
                  class="rounded-xl border p-4 text-left transition"
                  :class="outcome === 'yes' ? 'border-emerald-300 bg-emerald-500/15' : 'border-emerald-500/30 bg-emerald-500/10 hover:border-emerald-300'"
                  @click="outcome = 'yes'"
                >
                  <div class="mb-3 flex items-center justify-between gap-3">
                    <span class="font-bold text-emerald-300">{{ selectedMarket.localizedYesLabel }}</span>
                    <span class="font-mono text-2xl font-black text-emerald-200">{{ percentText(selectedMarket.yes_price) }}</span>
                  </div>
                  <div class="h-2 overflow-hidden rounded-full bg-emerald-950/70">
                    <div class="h-full rounded-full bg-emerald-300" :style="{ width: probabilityWidth(selectedMarket.yes_price) }"></div>
                  </div>
                </button>
                <button
                  type="button"
                  class="rounded-xl border p-4 text-left transition"
                  :class="outcome === 'no' ? 'border-rose-300 bg-rose-500/15' : 'border-rose-500/30 bg-rose-500/10 hover:border-rose-300'"
                  @click="outcome = 'no'"
                >
                  <div class="mb-3 flex items-center justify-between gap-3">
                    <span class="font-bold text-rose-300">{{ selectedMarket.localizedNoLabel }}</span>
                    <span class="font-mono text-2xl font-black text-rose-200">{{ percentText(selectedMarket.no_price) }}</span>
                  </div>
                  <div class="h-2 overflow-hidden rounded-full bg-rose-950/70">
                    <div class="h-full rounded-full bg-rose-300" :style="{ width: probabilityWidth(selectedMarket.no_price) }"></div>
                  </div>
                </button>
              </div>
            </section>

            <section class="rounded-xl border border-border bg-card/75 p-5">
              <h3 class="text-lg font-black">{{ t('prediction.rules') }}</h3>
              <p class="mt-2 text-sm leading-6 text-muted-foreground">{{ t('prediction.rules_desc') }}</p>
            </section>

            <section v-if="relatedMarkets.length > 0" class="rounded-xl border border-border bg-card/75 p-5">
              <h3 class="mb-3 text-lg font-black">{{ t('prediction.related_markets') }}</h3>
              <div class="grid gap-3 md:grid-cols-2">
                <button
                  v-for="market in relatedMarkets"
                  :key="market.id"
                  type="button"
                  class="rounded-lg border border-border bg-background/40 p-3 text-left transition hover:border-primary/50"
                  @click="openMarket(market)"
                >
                  <div class="line-clamp-2 text-sm font-bold">{{ market.localizedTitle }}</div>
                  <div class="mt-3 flex items-center justify-between gap-3 text-xs text-muted-foreground">
                    <span>{{ percentText(market.yes_price) }}</span>
                    <span>{{ formatCompactAmount(market.volume) }}</span>
                  </div>
                </button>
              </div>
            </section>
          </main>

          <aside class="rounded-xl border border-border bg-card/80 p-5 shadow-xl shadow-black/10 xl:sticky xl:top-24">
            <div class="flex flex-col gap-5">
              <div>
                <div class="text-xs font-bold text-primary">{{ t('prediction.order_ticket') }}</div>
                <h2 class="mt-1 line-clamp-3 text-lg font-black leading-snug">{{ selectedMarket.localizedTitle }}</h2>
              </div>

              <div class="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  class="rounded-xl border p-3 text-left transition"
                  :class="outcome === 'yes' ? 'border-emerald-300 bg-emerald-500/15' : 'border-border bg-muted/40 hover:border-emerald-500/40'"
                  @click="outcome = 'yes'"
                >
                  <div class="text-xs font-bold text-muted-foreground">{{ selectedMarket.localizedYesLabel }}</div>
                  <div class="mt-1 font-mono text-2xl font-black text-emerald-200">{{ percentText(selectedMarket.yes_price) }}</div>
                </button>
                <button
                  type="button"
                  class="rounded-xl border p-3 text-left transition"
                  :class="outcome === 'no' ? 'border-rose-300 bg-rose-500/15' : 'border-border bg-muted/40 hover:border-rose-500/40'"
                  @click="outcome = 'no'"
                >
                  <div class="text-xs font-bold text-muted-foreground">{{ selectedMarket.localizedNoLabel }}</div>
                  <div class="mt-1 font-mono text-2xl font-black text-rose-200">{{ percentText(selectedMarket.no_price) }}</div>
                </button>
              </div>

              <div class="space-y-3">
                <label class="block">
                  <span class="mb-2 block text-sm font-bold text-muted-foreground">{{ t('prediction.stake_asset') }}</span>
                  <div class="relative" @click.stop>
                    <button
                      type="button"
                      class="flex h-14 w-full items-center gap-3 rounded-xl border border-border bg-background px-3 text-left transition hover:border-primary disabled:cursor-not-allowed disabled:opacity-50"
                      :disabled="stakeAssetOptions.length === 0"
                      @click="assetSelectorOpen = !assetSelectorOpen"
                    >
                      <PairLogo class="h-9 w-9 rounded-lg" :symbol="selectedStakeAsset?.asset_symbol || '--'" :src="selectedStakeAsset?.logoUrl" />
                      <span class="min-w-0 flex-1">
                        <span class="block truncate font-bold text-foreground">{{ selectedStakeAsset?.asset_symbol || '--' }}</span>
                        <span class="block truncate text-xs text-muted-foreground">{{ t('prediction.available_balance') }} {{ selectedAssetBalanceText }}</span>
                      </span>
                      <Icon icon="mdi:chevron-down" class="h-4 w-4 text-muted-foreground transition" :class="{ 'rotate-180': assetSelectorOpen }" />
                    </button>

                    <div v-if="assetSelectorOpen" class="absolute left-0 right-0 top-full z-50 mt-2 rounded-xl border border-border bg-popover p-3 shadow-xl">
                      <div class="relative mb-3">
                        <Icon icon="mdi:magnify" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                        <input
                          v-model="assetSearchText"
                          type="search"
                          class="h-10 w-full rounded-lg border border-input bg-background pl-9 pr-3 text-sm outline-none focus:border-primary"
                          :placeholder="t('prediction.search_asset')"
                        />
                      </div>
                      <div class="max-h-72 space-y-1 overflow-y-auto">
                        <button
                          v-for="asset in filteredStakeAssetOptions"
                          :key="asset.asset_id"
                          type="button"
                          class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition hover:bg-muted"
                          @click="selectStakeAsset(asset)"
                        >
                          <PairLogo class="h-9 w-9 rounded-lg" :symbol="asset.asset_symbol" :src="asset.logoUrl" />
                          <span class="min-w-0 flex-1">
                            <span class="block font-bold">{{ asset.asset_symbol }}</span>
                            <span class="block truncate text-xs text-muted-foreground">{{ t('prediction.available_balance') }} {{ formatAmount(asset.balance) }} {{ asset.asset_symbol }}</span>
                          </span>
                          <Icon v-if="String(asset.asset_id) === assetId" icon="mdi:check" class="h-4 w-4 text-primary" />
                        </button>
                        <div v-if="filteredStakeAssetOptions.length === 0" class="py-8 text-center text-sm text-muted-foreground">
                          {{ t('prediction.no_assets') }}
                        </div>
                      </div>
                    </div>
                  </div>
                </label>

                <label class="block">
                  <span class="mb-2 flex items-center justify-between gap-3 text-sm font-bold text-muted-foreground">
                    <span>{{ t('prediction.stake_amount') }}</span>
                    <button type="button" class="text-xs text-primary hover:opacity-80" @click="setMaxStakeAmount">
                      {{ t('prediction.available_balance') }} {{ selectedAssetBalanceText }}
                    </button>
                  </span>
                  <div class="rounded-xl border border-border bg-background p-3 focus-within:border-primary">
                    <div class="flex items-center gap-3">
                      <input
                        v-model="stakeAmount"
                        class="min-w-0 flex-1 bg-transparent font-mono text-2xl font-black text-foreground outline-none placeholder:text-muted-foreground/50"
                        inputmode="decimal"
                        :placeholder="t('prediction.amount_placeholder')"
                      />
                      <span class="inline-flex items-center gap-2 rounded-full bg-muted px-3 py-1.5 text-sm font-bold text-muted-foreground">
                        <PairLogo class="h-5 w-5 rounded-full" :symbol="selectedStakeAsset?.asset_symbol || '--'" :src="selectedStakeAsset?.logoUrl" />
                        {{ selectedStakeAsset?.asset_symbol || '--' }}
                      </span>
                    </div>
                    <div class="mt-3 flex gap-2">
                      <button
                        v-for="amount in quickStakeAmounts"
                        :key="amount"
                        type="button"
                        class="rounded-md bg-muted px-2.5 py-1 text-xs font-bold text-muted-foreground transition hover:text-primary"
                        @click="stakeAmount = amount"
                      >
                        {{ amount }}
                      </button>
                    </div>
                    <div class="mt-3 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
                      <span>{{ t('prediction.max_stake') }} {{ maxStakeAmountText }}</span>
                      <span>{{ t('prediction.payout_cap') }} {{ payoutCapText }}</span>
                    </div>
                  </div>
                  <p v-if="stakeAmountError" class="mt-2 text-xs font-semibold text-destructive">{{ stakeAmountError }}</p>
                </label>
              </div>

              <div class="rounded-xl border border-border bg-background/50 p-4 text-sm">
                <div class="mb-3 flex items-center justify-between gap-3">
                  <span class="font-bold">{{ t('prediction.quote_snapshot') }}</span>
                  <span class="rounded-full px-2 py-1 text-xs font-bold" :class="outcome === 'yes' ? 'bg-emerald-500/15 text-emerald-300' : 'bg-rose-500/15 text-rose-300'">
                    {{ selectedOutcomeLabel }}
                  </span>
                </div>
                <div class="space-y-2">
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.price') }}</span><span class="font-mono">{{ quote ? percentText(quote.accepted_price) : percentText(selectedPrice) }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.stake') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.stake_amount)} ${quote.asset_symbol}` : '--' }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.fee') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.fee_amount)} ${quote.asset_symbol}` : '--' }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.shares') }}</span><span class="font-mono">{{ quote ? formatAmount(quote.shares) : '--' }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.max_payout') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.theoretical_payout)} ${quote.asset_symbol}` : '--' }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.payout_cap') }}</span><span class="font-mono">{{ quote ? formatPayoutCapAmount(quote.effective_payout_cap, quote.asset_symbol) : '--' }}</span></div>
                  <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.quote_expires') }}</span><span class="font-mono">{{ quoteExpiresText }}</span></div>
                </div>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <button
                  type="button"
                  class="flex h-12 items-center justify-center rounded-xl bg-muted px-4 font-bold text-foreground transition hover:bg-muted/80 disabled:cursor-not-allowed disabled:opacity-50"
                  :disabled="quoteLoading || !canQuote"
                  @click="requestQuote"
                >
                  <Icon v-if="quoteLoading" icon="mdi:loading" class="mr-2 h-4 w-4 animate-spin" />
                  {{ quoteLoading ? t('prediction.quoting') : t('prediction.get_quote') }}
                </button>
                <button
                  type="button"
                  class="flex h-12 items-center justify-center rounded-xl bg-primary px-4 font-black text-primary-foreground transition hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="orderLoading || !quote"
                  @click="submitOrder"
                >
                  <Icon v-if="orderLoading" icon="mdi:loading" class="mr-2 h-4 w-4 animate-spin" />
                  {{ orderLoading ? t('prediction.submitting') : t('prediction.place_order') }}
                </button>
              </div>
            </div>
          </aside>
        </div>
      </section>
    </div>
  </div>
</template>
