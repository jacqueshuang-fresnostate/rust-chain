<template>
  <div class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-8">
    <section class="flex flex-col gap-2">
      <div class="text-sm font-semibold text-primary">{{ t('prediction.badge') }}</div>
      <h1 class="text-3xl font-black text-foreground md:text-4xl">{{ t('prediction.title') }}</h1>
      <p class="max-w-3xl text-sm text-muted-foreground md:text-base">{{ t('prediction.subtitle') }}</p>
    </section>

    <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <section class="rounded-xl border border-border bg-card">
        <div class="flex flex-col gap-3 border-b border-border p-4 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 class="text-lg font-bold text-foreground">{{ t('prediction.markets') }}</h2>
            <p class="text-xs text-muted-foreground">{{ t('prediction.market_count', { count: markets.length }) }}</p>
          </div>
          <button class="rounded-lg bg-muted px-3 py-2 text-sm font-semibold text-foreground hover:bg-muted/80" :disabled="loading" @click="loadPage">
            {{ t('prediction.refresh') }}
          </button>
        </div>

        <div v-if="loading" class="p-8 text-center text-muted-foreground">{{ t('common.loading') }}</div>
        <div v-else-if="markets.length === 0" class="p-8 text-center text-muted-foreground">{{ t('prediction.no_markets') }}</div>
        <div v-else class="divide-y divide-border">
          <button
            v-for="market in markets"
            :key="market.id"
            type="button"
            class="flex w-full flex-col gap-4 p-4 text-left transition-colors hover:bg-muted/40 md:flex-row md:items-center md:justify-between"
            :class="{ 'bg-primary/5': selectedMarket?.id === market.id }"
            @click="selectMarket(market)"
          >
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-3">
                <img v-if="market.image_url" :src="market.image_url" :alt="market.title" loading="lazy" class="h-12 w-12 rounded-lg object-cover" />
                <div class="min-w-0">
                  <div class="truncate text-base font-bold text-foreground">{{ market.title }}</div>
                  <div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                    <span>{{ market.category || t('prediction.general') }}</span>
                    <span v-if="market.end_at">{{ formatTime(market.end_at) }}</span>
                    <span>{{ t(`prediction.settlement_${market.settlement_status}`) }}</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="grid w-full grid-cols-2 gap-2 md:w-56">
              <div class="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2">
                <div class="text-xs text-emerald-400">{{ market.outcome_yes_label || 'YES' }}</div>
                <div class="font-mono text-lg font-bold text-emerald-300">{{ percentText(market.yes_price) }}</div>
              </div>
              <div class="rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2">
                <div class="text-xs text-rose-400">{{ market.outcome_no_label || 'NO' }}</div>
                <div class="font-mono text-lg font-bold text-rose-300">{{ percentText(market.no_price) }}</div>
              </div>
            </div>
          </button>
        </div>
      </section>

      <aside class="rounded-xl border border-border bg-card p-5">
        <div v-if="selectedMarket" class="flex flex-col gap-5">
          <div>
            <div class="text-xs font-semibold text-muted-foreground">{{ selectedMarket.category || t('prediction.market') }}</div>
            <h2 class="mt-1 text-xl font-black text-foreground">{{ selectedMarket.title }}</h2>
            <p v-if="selectedMarket.description" class="mt-2 line-clamp-4 text-sm text-muted-foreground">{{ selectedMarket.description }}</p>
          </div>

          <div class="grid grid-cols-2 gap-2">
            <button type="button" class="rounded-lg border px-3 py-3 text-left" :class="outcome === 'yes' ? 'border-emerald-400 bg-emerald-500/15' : 'border-border bg-muted/30'" @click="outcome = 'yes'">
              <div class="text-xs text-muted-foreground">{{ selectedMarket.outcome_yes_label || 'YES' }}</div>
              <div class="font-mono text-xl font-black text-emerald-300">{{ percentText(selectedMarket.yes_price) }}</div>
            </button>
            <button type="button" class="rounded-lg border px-3 py-3 text-left" :class="outcome === 'no' ? 'border-rose-400 bg-rose-500/15' : 'border-border bg-muted/30'" @click="outcome = 'no'">
              <div class="text-xs text-muted-foreground">{{ selectedMarket.outcome_no_label || 'NO' }}</div>
              <div class="font-mono text-xl font-black text-rose-300">{{ percentText(selectedMarket.no_price) }}</div>
            </button>
          </div>

          <label class="flex flex-col gap-2">
            <span class="text-sm font-semibold text-muted-foreground">{{ t('prediction.stake_asset') }}</span>
            <select v-model="assetId" class="rounded-lg border border-border bg-background px-3 py-3 text-foreground">
              <option v-for="asset in effectiveAssets" :key="asset.asset_id" :value="String(asset.asset_id)">
                {{ asset.asset_symbol }}
              </option>
            </select>
          </label>

          <label class="flex flex-col gap-2">
            <span class="text-sm font-semibold text-muted-foreground">{{ t('prediction.stake_amount') }}</span>
            <input v-model="stakeAmount" class="rounded-lg border border-border bg-background px-3 py-3 font-mono text-foreground" inputmode="decimal" placeholder="0.00" />
          </label>

          <div class="rounded-lg bg-muted/40 p-4 text-sm">
            <div class="flex justify-between"><span class="text-muted-foreground">{{ t('prediction.price') }}</span><span class="font-mono">{{ percentText(selectedPrice) }}</span></div>
            <div class="mt-2 flex justify-between"><span class="text-muted-foreground">{{ t('prediction.fee') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.fee_amount)} ${quote.asset_symbol}` : '--' }}</span></div>
            <div class="mt-2 flex justify-between"><span class="text-muted-foreground">{{ t('prediction.shares') }}</span><span class="font-mono">{{ quote ? formatAmount(quote.shares) : '--' }}</span></div>
            <div class="mt-2 flex justify-between"><span class="text-muted-foreground">{{ t('prediction.max_payout') }}</span><span class="font-mono">{{ quote ? `${formatAmount(quote.theoretical_payout)} ${quote.asset_symbol}` : '--' }}</span></div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <button class="rounded-lg bg-muted px-4 py-3 font-bold text-foreground hover:bg-muted/80 disabled:opacity-50" :disabled="quoteLoading || !canQuote" @click="requestQuote">
              {{ quoteLoading ? t('prediction.quoting') : t('prediction.get_quote') }}
            </button>
            <button class="rounded-lg bg-primary px-4 py-3 font-black text-primary-foreground hover:bg-primary/90 disabled:opacity-50" :disabled="orderLoading || !quote" @click="submitOrder">
              {{ orderLoading ? t('prediction.submitting') : t('prediction.place_order') }}
            </button>
          </div>
        </div>
        <div v-else class="p-8 text-center text-muted-foreground">{{ t('prediction.select_market') }}</div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import { useRouter } from 'vue-router'
import { createPredictionOrder, createPredictionQuote, fetchPredictionConfig, fetchPredictionMarkets, type PredictionMarket, type PredictionOutcome, type PredictionQuote, type PredictionStakeAsset } from '@/api/prediction'
import { useUserStore } from '@/stores/user'
import { formatNumber } from '@/utils/format'

const { t } = useI18n()
const toast = useToast()
const router = useRouter()
const userStore = useUserStore()

const markets = ref<PredictionMarket[]>([])
const allowedAssets = ref<PredictionStakeAsset[]>([])
const selectedMarket = ref<PredictionMarket | null>(null)
const outcome = ref<PredictionOutcome>('yes')
const assetId = ref('')
const stakeAmount = ref('')
const quote = ref<PredictionQuote | null>(null)
const loading = ref(false)
const quoteLoading = ref(false)
const orderLoading = ref(false)

const selectedPrice = computed(() => {
  const market = selectedMarket.value
  if (!market) return '0'
  return outcome.value === 'yes' ? market.yes_price : market.no_price
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

const canQuote = computed(() => Boolean(selectedMarket.value && assetId.value && Number(stakeAmount.value) > 0))

watch([selectedMarket, outcome, assetId, stakeAmount], () => {
  quote.value = null
})

watch(effectiveAssets, (assets) => {
  if (!assets.some(asset => String(asset.asset_id) === assetId.value)) {
    assetId.value = assets[0] ? String(assets[0].asset_id) : ''
  }
}, { immediate: true })

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
    selectedMarket.value = markets.value[0] ?? null
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.load_failed')))
  } finally {
    loading.value = false
  }
}

function selectMarket(market: PredictionMarket) {
  selectedMarket.value = market
  outcome.value = 'yes'
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

function percentText(value: string | number) {
  const number = Number(value)
  if (!Number.isFinite(number)) return '--'
  return `${formatNumber(number * 100, 'amount')}%`
}

function formatAmount(value: string | number) {
  return formatNumber(value, 'amount')
}

function formatTime(value?: number | null) {
  if (!value) return '--'
  return new Date(value).toLocaleString()
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>
