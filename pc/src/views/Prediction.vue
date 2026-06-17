<template>
  <div class="h-full overflow-y-auto bg-background text-foreground">
    <div class="mx-auto flex w-full max-w-7xl flex-col gap-5 px-4 py-6 lg:px-6">
      <section class="rounded-xl border border-border bg-card/70 p-5 shadow-lg shadow-black/10">
        <div class="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div class="min-w-0">
            <div class="mb-2 inline-flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1 text-xs font-bold text-primary">
              <Icon icon="mdi:chart-timeline-variant-shimmer" class="h-4 w-4" />
              {{ t('prediction.badge') }}
            </div>
            <h1 class="text-3xl font-black leading-tight md:text-4xl">{{ t('prediction.title') }}</h1>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{{ t('prediction.subtitle') }}</p>
          </div>

          <div class="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:min-w-[520px]">
            <div v-for="item in summaryCards" :key="item.key" class="rounded-xl border border-border bg-background/50 p-3">
              <div class="mb-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
                <span>{{ item.label }}</span>
                <Icon :icon="item.icon" class="h-4 w-4 text-primary" />
              </div>
              <div class="truncate font-mono text-lg font-black">{{ item.value }}</div>
            </div>
          </div>
        </div>
      </section>

      <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(360px,420px)] xl:items-start">
        <section class="min-w-0 rounded-xl border border-border bg-card/70 shadow-xl shadow-black/10">
          <div class="border-b border-border p-4">
            <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div>
                <h2 class="text-xl font-black">{{ t('prediction.markets') }}</h2>
                <p class="text-sm text-muted-foreground">{{ t('prediction.market_count', { count: filteredMarkets.length }) }}</p>
              </div>
              <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
                <div class="relative sm:w-80">
                  <Icon icon="lucide:search" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <input
                    v-model="searchText"
                    type="search"
                    :placeholder="t('prediction.search_placeholder')"
                    class="h-10 w-full rounded-lg border border-input bg-background/70 pl-9 pr-3 text-sm outline-none transition-colors focus:border-primary"
                  />
                </div>
                <button
                  type="button"
                  class="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-border px-3 text-sm font-bold text-muted-foreground transition-colors hover:border-primary hover:text-primary disabled:opacity-50"
                  :disabled="loading"
                  @click="loadPage"
                >
                  <Icon icon="mdi:refresh" class="h-4 w-4" :class="{ 'animate-spin': loading }" />
                  {{ t('prediction.refresh') }}
                </button>
              </div>
            </div>

            <div class="mt-4 flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
              <div class="flex gap-2 overflow-x-auto pb-1">
                <button
                  v-for="category in categoryFilters"
                  :key="category.key"
                  type="button"
                  class="whitespace-nowrap rounded-lg px-3 py-2 text-xs font-bold transition-colors"
                  :class="activeCategory === category.key ? 'bg-primary text-primary-foreground shadow-lg shadow-primary/20' : 'bg-muted/60 text-muted-foreground hover:text-foreground'"
                  @click="activeCategory = category.key"
                >
                  {{ category.label }}
                </button>
              </div>

              <div class="flex flex-wrap gap-2">
                <button
                  v-for="option in sortOptions"
                  :key="option.key"
                  type="button"
                  class="rounded-lg border border-border px-3 py-1.5 text-xs font-bold transition-colors hover:border-primary hover:text-primary"
                  :class="sortKey === option.key ? 'border-primary bg-primary/10 text-primary' : 'text-muted-foreground'"
                  @click="sortKey = option.key"
                >
                  {{ t(option.labelKey) }}
                </button>
              </div>
            </div>
          </div>

          <div v-if="loading" class="flex items-center justify-center gap-2 px-4 py-20 text-sm text-muted-foreground">
            <Icon icon="mdi:loading" class="h-5 w-5 animate-spin text-primary" />
            {{ t('common.loading') }}
          </div>

          <div v-else-if="filteredMarkets.length === 0" class="px-4 py-16">
            <div class="rounded-xl border border-dashed border-border p-10 text-center">
              <Icon icon="mdi:archive-search-outline" class="mx-auto mb-3 h-8 w-8 text-muted-foreground" />
              <div class="font-bold">{{ t('prediction.no_markets') }}</div>
              <button
                v-if="searchText || activeCategory !== 'all'"
                type="button"
                class="mt-3 text-sm font-bold text-primary hover:opacity-80"
                @click="clearFilters"
              >
                {{ t('prediction.clear_filters') }}
              </button>
            </div>
          </div>

          <div v-else class="divide-y divide-border/70">
            <article
              v-for="market in filteredMarkets"
              :key="market.id"
              class="group grid gap-4 p-4 transition-colors hover:bg-muted/30 lg:grid-cols-[minmax(0,1fr)_260px]"
              :class="{ 'bg-primary/5': selectedMarket?.id === market.id }"
            >
              <button type="button" class="min-w-0 text-left" @click="selectMarket(market)">
                <div class="flex gap-3">
                  <div class="relative h-16 w-16 shrink-0 overflow-hidden rounded-xl border border-border bg-background/60">
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
                      <span v-if="market.end_at" class="inline-flex items-center gap-1 rounded-md bg-muted/60 px-2 py-1 text-xs text-muted-foreground">
                        <Icon icon="mdi:clock-outline" class="h-3.5 w-3.5" />
                        {{ formatDateTime(market.end_at) }}
                      </span>
                    </div>
                    <h3 class="line-clamp-2 text-base font-black leading-snug text-foreground group-hover:text-primary">
                      {{ market.localizedTitle }}
                    </h3>
                    <p v-if="market.localizedDescription" class="mt-2 line-clamp-2 text-sm text-muted-foreground">
                      {{ market.localizedDescription }}
                    </p>
                    <div class="mt-3 flex flex-wrap gap-4 text-xs text-muted-foreground">
                      <span>{{ t('prediction.volume') }} {{ formatCompactAmount(market.volume) }}</span>
                      <span>{{ t('prediction.liquidity') }} {{ formatCompactAmount(market.liquidity) }}</span>
                    </div>
                  </div>
                </div>
              </button>

              <div class="grid grid-cols-2 gap-2 self-center">
                <button
                  type="button"
                  class="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-3 text-left transition hover:border-emerald-300 hover:bg-emerald-500/15"
                  :class="{ 'ring-2 ring-emerald-300/40': selectedMarket?.id === market.id && outcome === 'yes' }"
                  @click="selectMarket(market, 'yes')"
                >
                  <div class="mb-2 flex items-center justify-between gap-2">
                    <span class="truncate text-xs font-bold text-emerald-300">{{ market.localizedYesLabel }}</span>
                    <Icon icon="mdi:arrow-up" class="h-4 w-4 text-emerald-300" />
                  </div>
                  <div class="font-mono text-xl font-black text-emerald-200">{{ percentText(market.yes_price) }}</div>
                  <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-emerald-950/70">
                    <div class="h-full rounded-full bg-emerald-300" :style="{ width: probabilityWidth(market.yes_price) }"></div>
                  </div>
                </button>
                <button
                  type="button"
                  class="rounded-xl border border-rose-500/30 bg-rose-500/10 p-3 text-left transition hover:border-rose-300 hover:bg-rose-500/15"
                  :class="{ 'ring-2 ring-rose-300/40': selectedMarket?.id === market.id && outcome === 'no' }"
                  @click="selectMarket(market, 'no')"
                >
                  <div class="mb-2 flex items-center justify-between gap-2">
                    <span class="truncate text-xs font-bold text-rose-300">{{ market.localizedNoLabel }}</span>
                    <Icon icon="mdi:arrow-down" class="h-4 w-4 text-rose-300" />
                  </div>
                  <div class="font-mono text-xl font-black text-rose-200">{{ percentText(market.no_price) }}</div>
                  <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-rose-950/70">
                    <div class="h-full rounded-full bg-rose-300" :style="{ width: probabilityWidth(market.no_price) }"></div>
                  </div>
                </button>
              </div>
            </article>
          </div>
        </section>

        <aside class="rounded-xl border border-border bg-card/80 p-5 shadow-xl shadow-black/10 xl:sticky xl:top-24">
          <div v-if="selectedMarket" class="flex flex-col gap-5">
            <div class="flex items-start gap-3">
              <div class="h-12 w-12 shrink-0 overflow-hidden rounded-xl border border-border bg-background/60">
                <img v-if="selectedMarket.image_url" :src="selectedMarket.image_url" :alt="selectedMarket.localizedTitle" loading="lazy" class="h-full w-full object-cover" />
                <div v-else class="flex h-full w-full items-center justify-center font-black text-primary">
                  {{ selectedMarket.localizedTitle.slice(0, 1) || '?' }}
                </div>
              </div>
              <div class="min-w-0">
                <div class="text-xs font-bold text-primary">{{ t('prediction.order_ticket') }}</div>
                <h2 class="mt-1 line-clamp-3 text-lg font-black leading-snug">{{ selectedMarket.localizedTitle }}</h2>
              </div>
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
                <div class="relative">
                  <select v-model="assetId" class="h-12 w-full appearance-none rounded-xl border border-border bg-background px-4 pr-10 font-bold text-foreground outline-none focus:border-primary">
                    <option v-for="asset in effectiveAssets" :key="asset.asset_id" :value="String(asset.asset_id)">
                      {{ asset.asset_symbol }}
                    </option>
                  </select>
                  <Icon icon="mdi:chevron-down" class="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                </div>
              </label>

              <label class="block">
                <span class="mb-2 block text-sm font-bold text-muted-foreground">{{ t('prediction.stake_amount') }}</span>
                <div class="rounded-xl border border-border bg-background p-3 focus-within:border-primary">
                  <div class="flex items-center gap-3">
                    <input
                      v-model="stakeAmount"
                      class="min-w-0 flex-1 bg-transparent font-mono text-2xl font-black text-foreground outline-none placeholder:text-muted-foreground/50"
                      inputmode="decimal"
                      :placeholder="t('prediction.amount_placeholder')"
                    />
                    <span class="inline-flex items-center gap-2 rounded-full bg-muted px-3 py-1.5 text-sm font-bold text-muted-foreground">
                      <PairLogo class="h-5 w-5 rounded-full" :symbol="selectedAsset?.asset_symbol || '--'" />
                      {{ selectedAsset?.asset_symbol || '--' }}
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
                </div>
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
                <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.price') }}</span><span class="font-mono">{{ percentText(selectedPrice) }}</span></div>
                <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.fee') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.fee_amount)} ${quote.asset_symbol}` : '--' }}</span></div>
                <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.shares') }}</span><span class="font-mono">{{ quote ? formatAmount(quote.shares) : '--' }}</span></div>
                <div class="flex justify-between gap-3"><span class="text-muted-foreground">{{ t('prediction.max_payout') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.theoretical_payout)} ${quote.asset_symbol}` : '--' }}</span></div>
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

          <div v-else class="rounded-xl border border-dashed border-border px-4 py-12 text-center text-sm text-muted-foreground">
            <Icon icon="mdi:cursor-default-click-outline" class="mx-auto mb-3 h-8 w-8" />
            {{ t('prediction.select_market') }}
          </div>
        </aside>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import { useRouter } from 'vue-router'
import { Icon } from '@iconify/vue'
import { createPredictionOrder, createPredictionQuote, fetchPredictionConfig, fetchPredictionMarkets, type PredictionMarket, type PredictionOutcome, type PredictionQuote, type PredictionStakeAsset } from '@/api/prediction'
import PairLogo from '@/components/common/PairLogo.vue'
import { useUserStore } from '@/stores/user'
import { formatNumber } from '@/utils/format'
import { localizePredictionMarketText } from '@/utils/predictionLocale'

type SortKey = 'popular' | 'volume' | 'ending'
type LocalizedPredictionMarket = PredictionMarket & {
  localizedTitle: string
  localizedDescription: string
  localizedCategory: string
  localizedYesLabel: string
  localizedNoLabel: string
  searchIndex: string
}

const { t, locale } = useI18n()
const toast = useToast()
const router = useRouter()
const userStore = useUserStore()

const markets = ref<PredictionMarket[]>([])
const allowedAssets = ref<PredictionStakeAsset[]>([])
const selectedMarketId = ref<number | null>(null)
const outcome = ref<PredictionOutcome>('yes')
const assetId = ref('')
const stakeAmount = ref('')
const quote = ref<PredictionQuote | null>(null)
const loading = ref(false)
const quoteLoading = ref(false)
const orderLoading = ref(false)
const searchText = ref('')
const activeCategory = ref('all')
const sortKey = ref<SortKey>('popular')

const quickStakeAmounts = ['25', '50', '100']
const sortOptions: Array<{ key: SortKey; labelKey: string }> = [
  { key: 'popular', labelKey: 'prediction.sort_popular' },
  { key: 'volume', labelKey: 'prediction.sort_volume' },
  { key: 'ending', labelKey: 'prediction.sort_ending' },
]

const localizedMarkets = computed<LocalizedPredictionMarket[]>(() => {
  const currentLocale = String(locale.value || '')
  return markets.value.map((market) => {
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
  })
})

const selectedMarket = computed(() => localizedMarkets.value.find((market) => market.id === selectedMarketId.value) ?? localizedMarkets.value[0] ?? null)

const selectedPrice = computed(() => {
  const market = selectedMarket.value
  if (!market) return '0'
  return outcome.value === 'yes' ? market.yes_price : market.no_price
})

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

const selectedAsset = computed(() => effectiveAssets.value.find((asset) => String(asset.asset_id) === assetId.value) ?? effectiveAssets.value[0] ?? null)

const canQuote = computed(() => Boolean(selectedMarket.value && assetId.value && Number(stakeAmount.value) > 0 && effectiveAssets.value.length > 0))

const categoryFilters = computed(() => {
  const categories = Array.from(new Set(localizedMarkets.value.map((market) => market.localizedCategory).filter(Boolean))).sort()
  return [
    { key: 'all', label: t('prediction.all_categories') },
    ...categories.map((category) => ({ key: category, label: category })),
  ]
})

const filteredMarkets = computed(() => {
  const keyword = searchText.value.trim().toLowerCase()
  const filtered = localizedMarkets.value.filter((market) => {
    if (activeCategory.value !== 'all' && market.localizedCategory !== activeCategory.value) return false
    if (keyword && !market.searchIndex.includes(keyword)) return false
    return true
  })
  return sortMarkets(filtered, sortKey.value)
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
const endingSoonCount = computed(() => {
  const now = Date.now()
  const sevenDays = 7 * 24 * 60 * 60 * 1000
  return localizedMarkets.value.filter((market) => {
    const endAt = Number(market.end_at || 0)
    return endAt > now && endAt - now <= sevenDays
  }).length
})

const quoteExpiresText = computed(() => quote.value?.expires_at ? formatDateTime(quote.value.expires_at) : '--')

watch([selectedMarketId, outcome, assetId, stakeAmount], () => {
  quote.value = null
})

watch(effectiveAssets, (assets) => {
  if (!assets.some(asset => String(asset.asset_id) === assetId.value)) {
    assetId.value = assets[0] ? String(assets[0].asset_id) : ''
  }
}, { immediate: true })

watch(categoryFilters, (filters) => {
  if (!filters.some((filter) => filter.key === activeCategory.value)) {
    activeCategory.value = 'all'
  }
})

watch(selectedMarket, (market) => {
  if (market && selectedMarketId.value !== market.id) {
    selectedMarketId.value = market.id
  }
})

onMounted(loadPage)

async function loadPage() {
  loading.value = true
  try {
    const [configResponse, marketResponse] = await Promise.all([
      fetchPredictionConfig(),
      fetchPredictionMarkets(),
    ])
    allowedAssets.value = configResponse.data.allowed_assets
    markets.value = marketResponse.data
    if (!markets.value.some((market) => market.id === selectedMarketId.value)) {
      selectedMarketId.value = markets.value[0]?.id ?? null
    }
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.load_failed')))
  } finally {
    loading.value = false
  }
}

function selectMarket(market: LocalizedPredictionMarket, nextOutcome?: PredictionOutcome) {
  selectedMarketId.value = market.id
  if (nextOutcome) {
    outcome.value = nextOutcome
  }
}

function clearFilters() {
  searchText.value = ''
  activeCategory.value = 'all'
}

async function requestQuote() {
  if (!canQuote.value || !selectedMarket.value) return
  if (!userStore.isLoggedIn) {
    await router.push('/login')
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

function sortMarkets(items: LocalizedPredictionMarket[], sort: SortKey) {
  const marketsToSort = [...items]
  if (sort === 'volume') {
    return marketsToSort.sort((a, b) => numberValue(b.volume) - numberValue(a.volume))
  }
  if (sort === 'ending') {
    return marketsToSort.sort((a, b) => dateSortValue(a.end_at) - dateSortValue(b.end_at))
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

function dateSortValue(value?: number | null) {
  const number = Number(value || 0)
  return Number.isFinite(number) && number > 0 ? number : Number.MAX_SAFE_INTEGER
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
