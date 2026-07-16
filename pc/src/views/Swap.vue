<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'

import {
  confirmSwapQuote,
  fetchSwapBalances,
  fetchSwapPairs,
  requestSwapQuote,
} from '@/api/swap'
import type { PcSwapPairOption, PcSwapQuote, PcTradeWalletBalance } from '@/api/backendAdapters'
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import PairLogo from '@/components/common/PairLogo.vue'
import { useAuthRequired } from '@/composables/useAuthRequired'
import { formatNumber } from '@/utils/format'

type AssetSelectorKind = 'from' | 'to'

interface SwapAssetChoice {
  symbol: string
  balance: number
  logoUrl?: string
}

const { t } = useI18n()
const toast = useToast()
const { isLoggedIn, goToLogin } = useAuthRequired()

const pairs = ref<PcSwapPairOption[]>([])
const balances = ref<PcTradeWalletBalance[]>([])
const fromToken = ref('')
const toToken = ref('')
const fromAmount = ref('')
const quote = ref<PcSwapQuote | null>(null)
const loading = ref(false)
const loadingPairs = ref(false)
const quoteLoading = ref(false)
const quoteError = ref('')
const selectorOpen = ref<AssetSelectorKind | null>(null)
const assetSearch = ref<Record<AssetSelectorKind, string>>({ from: '', to: '' })

let quoteTimer: ReturnType<typeof setTimeout> | null = null
let quoteRequestId = 0

const pairOptions = computed(() => pairs.value.filter((pair) => pair.enabled))
const fromTokenOptions = computed(() => uniqueSorted(pairOptions.value.map((pair) => pair.fromUnit)))
const toTokenOptions = computed(() => uniqueSorted(pairOptions.value.filter((pair) => pair.fromUnit === fromToken.value).map((pair) => pair.toUnit)))
const selectedPair = computed(() => pairOptions.value.find((pair) => pair.fromUnit === fromToken.value && pair.toUnit === toToken.value) ?? null)
const walletBySymbol = computed(() => new Map(balances.value.map((balance) => [balance.symbol, balance])))
const fromAssetOptions = computed(() => buildAssetOptions(fromTokenOptions.value))
const toAssetOptions = computed(() => buildAssetOptions(toTokenOptions.value))
const filteredFromAssetOptions = computed(() => filterAssetOptions(fromAssetOptions.value, assetSearch.value.from))
const filteredToAssetOptions = computed(() => filterAssetOptions(toAssetOptions.value, assetSearch.value.to))
const selectedFromAsset = computed(() => assetChoice(fromToken.value))
const selectedToAsset = computed(() => assetChoice(toToken.value))
const amountNumber = computed(() => Number(fromAmount.value))
const fromBalance = computed(() => walletBySymbol.value.get(fromToken.value)?.balance ?? 0)
const toBalance = computed(() => walletBySymbol.value.get(toToken.value)?.balance ?? 0)
const minAmount = computed(() => selectedPair.value?.minAmount ?? 0)
const maxAmount = computed(() => selectedPair.value?.maxAmount ?? 0)
const expectedAmount = computed(() => quote.value?.toAmount ?? 0)
const exchangeRate = computed(() => quote.value?.rate ?? 0)
const canSubmit = computed(() => Boolean(isLoggedIn.value && selectedPair.value && amountNumber.value > 0 && !validationMessage.value && !loading.value && !quoteLoading.value))
const heroTitle = computed(() => t('swap.title', { from: fromToken.value || '--', to: toToken.value || '--' }))
const limitText = computed(() => selectedPair.value ? amountRangeText(true) : '--')
const fromAmountPlaceholder = computed(() => selectedPair.value ? amountRangeText(false) : '0.0')
const fromBalanceText = computed(() => isLoggedIn.value ? formatNumber(fromBalance.value, 'amount') : '--')
const toBalanceText = computed(() => isLoggedIn.value ? formatNumber(toBalance.value, 'amount') : '--')
const receiveAmountText = computed(() => {
  if (quoteLoading.value) return t('swap.quoting')
  if (!quote.value) return ''
  return formatNumber(expectedAmount.value, 'amount')
})
const rateText = computed(() => {
  if (!fromToken.value || !toToken.value || !exchangeRate.value) {
    return `1 ${fromToken.value || '--'} ≈ -- ${toToken.value || '--'}`
  }
  return `1 ${fromToken.value} ≈ ${formatNumber(exchangeRate.value, 'price')} ${toToken.value}`
})
const feeText = computed(() => {
  if (quoteLoading.value) return t('swap.quoting')
  if (!quote.value) return '--'
  return `${formatNumber(quote.value.feeAmount, 'amount')} ${fromToken.value}`
})
const validationMessage = computed(() => {
  if (!selectedPair.value) return t('swap.no_pair')
  if (!fromAmount.value) return ''
  if (!isLoggedIn.value) return t('common.login_required_title')
  if (!Number.isFinite(amountNumber.value) || amountNumber.value <= 0) return t('swap.invalid_amount')
  if (minAmount.value > 0 && amountNumber.value < minAmount.value) return `${t('swap.min_amount_error')} ${formatNumber(minAmount.value, 'amount')} ${fromToken.value}`
  if (maxAmount.value > 0 && amountNumber.value > maxAmount.value) return `${t('swap.max_amount_error')} ${formatNumber(maxAmount.value, 'amount')} ${fromToken.value}`
  if (amountNumber.value > fromBalance.value) return t('swap.insufficient_balance')
  return ''
})

watch(pairOptions, ensureSelection, { immediate: true })
watch(fromToken, () => {
  const firstTo = toTokenOptions.value[0] ?? ''
  if (!toTokenOptions.value.includes(toToken.value)) {
    toToken.value = firstTo
  }
  assetSearch.value.from = ''
})
watch(toToken, () => {
  assetSearch.value.to = ''
})
watch([selectedPair, fromAmount], scheduleQuote)
watch(isLoggedIn, (loggedIn) => {
  quote.value = null
  quoteError.value = ''
  if (loggedIn) {
    void refreshBalances()
    scheduleQuote()
  } else {
    balances.value = []
  }
})

onMounted(() => {
  loadSwap()
})

async function loadSwap() {
  loadingPairs.value = true
  try {
    const pairResponse = await fetchSwapPairs()
    pairs.value = pairResponse.data.data
    if (isLoggedIn.value) {
      await refreshBalances()
    } else {
      balances.value = []
    }
    ensureSelection()
  } catch (error) {
    toast.error(errorMessage(error, t('swap.load_failed')))
  } finally {
    loadingPairs.value = false
  }
}

async function refreshBalances() {
  if (!isLoggedIn.value) {
    balances.value = []
    return
  }
  const balanceResult = await fetchSwapBalances()
  balances.value = balanceResult.data.data
}

function ensureSelection() {
  if (fromToken.value && toToken.value && selectedPair.value) return
  const firstPair = pairOptions.value[0]
  fromToken.value = firstPair?.fromUnit ?? ''
  toToken.value = firstPair?.toUnit ?? ''
}

function scheduleQuote() {
  quote.value = null
  quoteError.value = ''
  if (quoteTimer) {
    clearTimeout(quoteTimer)
    quoteTimer = null
  }
  if (!isLoggedIn.value) return
  if (!selectedPair.value || validationMessage.value || !fromAmount.value) return
  quoteTimer = setTimeout(() => {
    refreshQuote()
  }, 500)
}

async function refreshQuote() {
  const pair = selectedPair.value
  if (!isLoggedIn.value) return
  if (!pair || validationMessage.value || amountNumber.value <= 0) return
  const requestId = ++quoteRequestId
  quoteLoading.value = true
  try {
    const response = await requestSwapQuote(pair, amountNumber.value)
    if (requestId === quoteRequestId) {
      quote.value = response.data.data
    }
  } catch (error) {
    if (requestId === quoteRequestId) {
      quoteError.value = errorMessage(error, t('swap.quote_failed'))
    }
  } finally {
    if (requestId === quoteRequestId) {
      quoteLoading.value = false
    }
  }
}

async function handleSwap() {
  if (!isLoggedIn.value) {
    goToLogin()
    return
  }
  if (!selectedPair.value) {
    toast.error(t('swap.no_pair'))
    return
  }
  if (validationMessage.value) {
    toast.error(validationMessage.value)
    return
  }
  loading.value = true
  try {
    const latestQuote = await requestSwapQuote(selectedPair.value, amountNumber.value)
    await confirmSwapQuote(latestQuote.data.data.quoteId)
    quote.value = latestQuote.data.data
    toast.success(t('swap.success'))
    fromAmount.value = ''
    quote.value = null
    await refreshBalances()
  } catch (error) {
    toast.error(errorMessage(error, t('swap.failed')))
  } finally {
    loading.value = false
  }
}

function switchTokens() {
  const reverse = pairOptions.value.find((pair) => pair.fromUnit === toToken.value && pair.toUnit === fromToken.value)
  if (!reverse) {
    toast.error(t('swap.reverse_unavailable'))
    return
  }
  fromToken.value = reverse.fromUnit
  toToken.value = reverse.toUnit
  fromAmount.value = ''
  quote.value = null
  selectorOpen.value = null
}

function setMaxAmount() {
  if (!isLoggedIn.value) {
    goToLogin()
    return
  }
  if (fromBalance.value <= 0) return
  const limit = maxAmount.value > 0 ? Math.min(fromBalance.value, maxAmount.value) : fromBalance.value
  fromAmount.value = String(limit)
}

function toggleSelector(kind: AssetSelectorKind) {
  selectorOpen.value = selectorOpen.value === kind ? null : kind
  if (selectorOpen.value === kind) {
    assetSearch.value[kind] = ''
  }
}

function selectAsset(kind: AssetSelectorKind, symbol: string) {
  if (kind === 'from') {
    fromToken.value = symbol
  } else {
    toToken.value = symbol
  }
  assetSearch.value[kind] = ''
  selectorOpen.value = null
  quote.value = null
  quoteError.value = ''
}

function buildAssetOptions(symbols: string[]): SwapAssetChoice[] {
  return symbols.map((symbol) => assetChoice(symbol))
}

function assetChoice(symbol: string): SwapAssetChoice {
  const normalized = symbol.toUpperCase()
  const wallet = walletBySymbol.value.get(normalized)
  return {
    symbol: normalized,
    balance: isLoggedIn.value ? (wallet?.balance ?? 0) : 0,
    logoUrl: wallet?.logoUrl,
  }
}

function assetBalanceText(balance: number): string {
  return isLoggedIn.value ? formatNumber(balance, 'amount') : '--'
}

function filterAssetOptions(options: SwapAssetChoice[], keyword: string): SwapAssetChoice[] {
  const normalized = keyword.trim().toUpperCase()
  if (!normalized) return options
  return options.filter((asset) => asset.symbol.includes(normalized))
}

function amountRangeText(withUnit: boolean): string {
  const suffix = withUnit ? ` ${fromToken.value}` : ''
  const min = formatNumber(minAmount.value, 'amount')
  if (maxAmount.value > 0) {
    return `${min} - ${formatNumber(maxAmount.value, 'amount')}${suffix}`
  }
  return `${t('swap.min')} ${min}${suffix}`
}

function formatTime(value: number): string {
  if (!value) return '--'
  return new Date(value).toLocaleString()
}

function uniqueSorted(values: string[]): string[] {
  return Array.from(new Set(values.filter(Boolean).map((value) => value.toUpperCase()))).sort()
}

function errorMessage(error: unknown, fallback: string): string {
  const maybeError = error as { response?: { data?: { message?: string } }; message?: string }
  return maybeError.response?.data?.message || maybeError.message || fallback
}
</script>

<template>
  <div class="mx-auto max-w-6xl px-4 py-8 lg:py-14" @click="selectorOpen = null">
    <div class="grid grid-cols-1 gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(420px,500px)] lg:items-start">
      <section class="space-y-6 lg:pt-16">
        <div class="space-y-4">
          <div class="text-sm font-semibold text-primary">{{ t('nav.swap') }}</div>
          <h1 class="max-w-2xl text-4xl font-black leading-tight text-foreground lg:text-5xl">
            {{ heroTitle }}
          </h1>
        </div>
      </section>

      <form class="rounded-2xl border border-border bg-card p-5 shadow-sm lg:p-7" @submit.prevent="handleSwap">
        <div class="mb-7 flex items-center justify-between gap-4">
          <h2 class="text-2xl font-black">{{ t('nav.swap') }}</h2>
          <button
            type="button"
            class="inline-flex h-9 w-9 items-center justify-center rounded-full border border-border text-muted-foreground transition hover:text-primary disabled:opacity-50"
            :disabled="loadingPairs"
            :title="t('swap.refresh')"
            @click.stop="loadSwap"
          >
            <Icon icon="mdi:refresh" class="h-5 w-5" />
          </button>
        </div>

        <div v-if="loadingPairs" class="rounded-2xl bg-muted/60 py-20 text-center text-sm text-muted-foreground">
          {{ t('common.loading') }}
        </div>

        <div v-else class="space-y-0">
          <div class="rounded-2xl bg-muted/70 p-5">
            <div class="mb-4 flex items-center justify-between gap-3">
              <span class="text-sm font-medium text-muted-foreground">{{ t('swap.from') }}</span>
              <button type="button" class="text-sm font-medium text-primary hover:opacity-80" @click.stop="setMaxAmount">
                {{ t('swap.balance') }} {{ fromBalanceText }}
              </button>
            </div>

            <div class="flex items-center gap-3">
              <input
                v-model="fromAmount"
                min="0"
                step="any"
                type="number"
                class="min-w-0 flex-1 bg-transparent text-3xl font-bold outline-none placeholder:text-muted-foreground/60"
                :placeholder="fromAmountPlaceholder"
              />

              <div class="relative shrink-0" @click.stop>
                <button
                  type="button"
                  class="inline-flex h-12 min-w-32 items-center gap-2 rounded-full bg-background px-3 font-bold shadow-sm transition hover:bg-background/80 disabled:opacity-50"
                  :disabled="fromAssetOptions.length === 0"
                  @click="toggleSelector('from')"
                >
                  <PairLogo class="h-8 w-8 rounded-full" :symbol="selectedFromAsset.symbol || '--'" :src="selectedFromAsset.logoUrl" />
                  <span>{{ fromToken || '--' }}</span>
                  <Icon icon="mdi:chevron-down" class="h-4 w-4 text-muted-foreground transition" :class="{ 'rotate-180': selectorOpen === 'from' }" />
                </button>

                <div v-if="selectorOpen === 'from'" class="absolute right-0 top-full z-50 mt-2 w-80 max-w-[calc(100vw-2rem)] rounded-xl border border-border bg-popover p-3 shadow-xl">
                  <div class="relative mb-3">
                    <Icon icon="mdi:magnify" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <input
                      v-model="assetSearch.from"
                      type="text"
                      class="h-10 w-full rounded-lg border border-input bg-background pl-9 pr-3 text-sm outline-none focus:border-primary"
                      :placeholder="t('swap.search_asset')"
                    />
                  </div>
                  <div class="max-h-72 space-y-1 overflow-y-auto">
                    <button
                      v-for="asset in filteredFromAssetOptions"
                      :key="asset.symbol"
                      type="button"
                      class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition hover:bg-muted"
                      @click="selectAsset('from', asset.symbol)"
                    >
                      <PairLogo class="h-9 w-9 rounded-full" :symbol="asset.symbol" :src="asset.logoUrl" />
                      <span class="min-w-0 flex-1">
                        <span class="block font-bold">{{ asset.symbol }}</span>
                        <span class="block truncate text-xs text-muted-foreground">{{ t('swap.available') }} {{ assetBalanceText(asset.balance) }}</span>
                      </span>
                      <Icon v-if="asset.symbol === fromToken" icon="mdi:check" class="h-4 w-4 text-primary" />
                    </button>
                    <div v-if="filteredFromAssetOptions.length === 0" class="py-8 text-center text-sm text-muted-foreground">
                      {{ t('swap.no_assets') }}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="relative z-10 flex justify-center -my-3">
            <button
              type="button"
              class="inline-flex h-11 w-11 items-center justify-center rounded-full border border-border bg-card text-foreground shadow-sm transition hover:border-primary hover:text-primary"
              :title="t('swap.switch')"
              @click.stop="switchTokens"
            >
              <Icon icon="mdi:swap-vertical" class="h-5 w-5" />
            </button>
          </div>

          <div class="rounded-2xl bg-muted/70 p-5">
            <div class="mb-4 flex items-center justify-between gap-3">
              <span class="text-sm font-medium text-muted-foreground">{{ t('swap.to') }}</span>
              <span class="text-sm text-muted-foreground">{{ t('swap.balance') }} {{ toBalanceText }}</span>
            </div>

            <div class="flex items-center gap-3">
              <input
                :value="receiveAmountText"
                readonly
                type="text"
                class="min-w-0 flex-1 bg-transparent text-3xl font-bold text-muted-foreground outline-none placeholder:text-muted-foreground/60"
                :placeholder="t('swap.receive_placeholder')"
              />

              <div class="relative shrink-0" @click.stop>
                <button
                  type="button"
                  class="inline-flex h-12 min-w-32 items-center gap-2 rounded-full bg-background px-3 font-bold shadow-sm transition hover:bg-background/80 disabled:opacity-50"
                  :disabled="toAssetOptions.length === 0"
                  @click="toggleSelector('to')"
                >
                  <PairLogo class="h-8 w-8 rounded-full" :symbol="selectedToAsset.symbol || '--'" :src="selectedToAsset.logoUrl" />
                  <span>{{ toToken || '--' }}</span>
                  <Icon icon="mdi:chevron-down" class="h-4 w-4 text-muted-foreground transition" :class="{ 'rotate-180': selectorOpen === 'to' }" />
                </button>

                <div v-if="selectorOpen === 'to'" class="absolute right-0 top-full z-50 mt-2 w-80 max-w-[calc(100vw-2rem)] rounded-xl border border-border bg-popover p-3 shadow-xl">
                  <div class="relative mb-3">
                    <Icon icon="mdi:magnify" class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <input
                      v-model="assetSearch.to"
                      type="text"
                      class="h-10 w-full rounded-lg border border-input bg-background pl-9 pr-3 text-sm outline-none focus:border-primary"
                      :placeholder="t('swap.search_asset')"
                    />
                  </div>
                  <div class="max-h-72 space-y-1 overflow-y-auto">
                    <button
                      v-for="asset in filteredToAssetOptions"
                      :key="asset.symbol"
                      type="button"
                      class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition hover:bg-muted"
                      @click="selectAsset('to', asset.symbol)"
                    >
                      <PairLogo class="h-9 w-9 rounded-full" :symbol="asset.symbol" :src="asset.logoUrl" />
                      <span class="min-w-0 flex-1">
                        <span class="block font-bold">{{ asset.symbol }}</span>
                        <span class="block truncate text-xs text-muted-foreground">{{ t('swap.available') }} {{ assetBalanceText(asset.balance) }}</span>
                      </span>
                      <Icon v-if="asset.symbol === toToken" icon="mdi:check" class="h-4 w-4 text-primary" />
                    </button>
                    <div v-if="filteredToAssetOptions.length === 0" class="py-8 text-center text-sm text-muted-foreground">
                      {{ t('swap.no_assets') }}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="space-y-3 py-5 text-sm">
            <div class="flex items-center justify-between gap-3 text-muted-foreground">
              <span>{{ t('swap.rate') }}</span>
              <span class="text-right font-medium text-foreground">{{ rateText }}</span>
            </div>
            <div class="flex items-center justify-between gap-3 text-muted-foreground">
              <span>{{ t('swap.fee') }}</span>
              <span class="text-right">{{ feeText }}</span>
            </div>
            <div class="flex items-center justify-between gap-3 text-muted-foreground">
              <span>{{ t('swap.limit') }}</span>
              <span class="text-right">{{ limitText }}</span>
            </div>
            <div class="flex items-center justify-between gap-3 text-muted-foreground">
              <span>{{ t('swap.expires_at') }}</span>
              <span class="text-right">{{ quote ? formatTime(quote.expiresAt) : '--' }}</span>
            </div>
          </div>

          <p v-if="validationMessage" class="mb-4 text-sm text-down">{{ validationMessage }}</p>
          <p v-else-if="quoteError" class="mb-4 text-sm text-down">{{ quoteError }}</p>

          <AuthRequiredState v-if="!isLoggedIn" compact class="mb-4" />

          <button
            :type="isLoggedIn ? 'submit' : 'button'"
            :disabled="isLoggedIn && !canSubmit"
            class="flex h-14 w-full items-center justify-center rounded-xl bg-primary text-base font-bold text-primary-foreground transition hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-40"
            @click="!isLoggedIn ? goToLogin() : undefined"
          >
            <Icon v-if="loading" icon="mdi:loading" class="mr-2 h-5 w-5 animate-spin" />
            <span>{{ !isLoggedIn ? t('common.login_now') : loading ? t('swap.swapping') : t('swap.action') }}</span>
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
