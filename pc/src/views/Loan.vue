<template>
  <div class="min-h-full bg-background">
    <div class="mx-auto max-w-7xl px-4 py-8 lg:px-8">
      <div class="mb-6 flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <p class="text-sm font-semibold text-primary">{{ t('loan.title') }}</p>
          <h1 class="mt-2 text-3xl font-bold text-foreground">{{ t('loan.market_title') }}</h1>
          <p class="mt-2 max-w-2xl text-sm text-muted-foreground">{{ t('loan.market_subtitle') }}</p>
        </div>
        <RouterLink to="/user/loan-orders" class="inline-flex items-center gap-2 rounded-lg border border-border px-4 py-2 text-sm font-semibold hover:bg-muted/60">
          <Icon icon="mdi:file-document-outline" class="h-4 w-4" />
          {{ t('loan.my_orders') }}
        </RouterLink>
      </div>

      <div v-if="loading" class="rounded-xl border border-dashed border-border py-16 text-center text-muted-foreground">
        {{ t('common.loading') }}
      </div>

      <div v-else-if="products.length === 0" class="rounded-xl border border-dashed border-border py-16 text-center">
        <Icon icon="mdi:bank-off-outline" class="mx-auto mb-4 h-12 w-12 text-muted-foreground" />
        <h2 class="text-xl font-semibold">{{ t('loan.no_products') }}</h2>
        <p class="mt-2 text-sm text-muted-foreground">{{ t('loan.no_products_desc') }}</p>
      </div>

      <div v-else class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_420px]">
        <section class="space-y-3">
          <button
            v-for="product in products"
            :key="product.id"
            type="button"
            class="w-full rounded-xl border bg-card p-5 text-left transition hover:border-primary/70 hover:shadow-sm"
            :class="selectedProductId === product.id ? 'border-primary shadow-sm' : 'border-border'"
            @click="selectProduct(product)"
          >
            <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
              <div>
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="text-lg font-bold">{{ productName(product) }}</h3>
                  <span class="rounded-full bg-primary/10 px-2.5 py-1 text-xs font-semibold text-primary">{{ loanTypeText(product.loan_type) }}</span>
                  <span class="rounded-full bg-muted px-2.5 py-1 text-xs text-muted-foreground">{{ interestModeText(product.interest_calculation_mode) }}</span>
                </div>
                <p class="mt-2 text-sm text-muted-foreground">
                  {{ product.term_days }} {{ t('loan.days') }} · {{ t('loan.min_kyc_level', { level: product.min_kyc_level }) }}
                </p>
              </div>
              <div class="grid grid-cols-2 gap-4 text-right md:min-w-[280px]">
                <div>
                  <p class="text-xs text-muted-foreground">{{ t('loan.loan_asset') }}</p>
                  <p class="font-bold">{{ product.asset_symbol }}</p>
                </div>
                <div>
                  <p class="text-xs text-muted-foreground">{{ t('loan.rate') }}</p>
                  <p class="font-bold">{{ percentText(product.interest_rate) }}</p>
                </div>
                <div class="col-span-2">
                  <p class="text-xs text-muted-foreground">{{ t('loan.range') }}</p>
                  <p class="font-mono text-sm font-semibold">{{ amountRangeText(product) }} {{ product.asset_symbol }}</p>
                </div>
              </div>
            </div>
          </button>
        </section>

        <aside class="h-fit rounded-xl border border-border bg-card p-5 shadow-sm">
          <div class="mb-5 flex items-center justify-between gap-3">
            <div>
              <p class="text-sm text-muted-foreground">{{ t('loan.configure') }}</p>
              <h2 class="text-xl font-bold">{{ selectedProduct ? productName(selectedProduct) : '--' }}</h2>
            </div>
            <span class="rounded-full bg-muted px-3 py-1 text-xs font-semibold">{{ selectedProduct?.asset_symbol || '--' }}</span>
          </div>

          <div v-if="!userStore.isLoggedIn" class="mb-5 rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4 text-sm text-yellow-600">
            {{ t('loan.login_required_desc') }}
          </div>

          <div v-else-if="selectedProduct && !kycEligible" class="mb-5 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-500">
            {{ t('loan.kyc_required_desc', { level: selectedProduct.min_kyc_level, current: kycLevel }) }}
          </div>

          <div class="space-y-4">
            <label class="block">
              <span class="mb-1 block text-xs font-medium text-muted-foreground">{{ t('loan.amount') }}</span>
              <input v-model="amount" type="number" min="0" step="any" inputmode="decimal" class="w-full rounded-lg border border-border bg-background px-3 py-3 text-sm outline-none focus:border-primary" :placeholder="amountPlaceholder" />
            </label>

            <div v-if="requiresCollateral" class="rounded-lg border border-border bg-background/60 p-4">
              <p class="mb-3 text-sm font-semibold">{{ t('loan.collateral') }}</p>
              <div class="space-y-3">
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-muted-foreground">{{ t('loan.collateral_asset') }}</span>
                  <select v-model="collateralAssetId" class="w-full rounded-lg border border-border bg-background px-3 py-3 text-sm outline-none focus:border-primary">
                    <option value="">{{ t('loan.select_collateral_asset') }}</option>
                    <option v-for="wallet in walletAccounts" :key="wallet.id" :value="String(wallet.id)">
                      {{ wallet.coin.coinGroup }} · {{ t('loan.available') }} {{ formatAmount(wallet.balance) }}
                    </option>
                  </select>
                </label>
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-muted-foreground">{{ t('loan.collateral_amount') }}</span>
                  <div class="flex gap-2">
                    <input v-model="collateralAmount" type="number" min="0" step="any" inputmode="decimal" class="min-w-0 flex-1 rounded-lg border border-border bg-background px-3 py-3 text-sm outline-none focus:border-primary" placeholder="0.00" />
                    <button type="button" class="rounded-lg bg-muted px-3 text-sm font-semibold hover:bg-muted/80" @click="setMaxCollateral">{{ t('loan.max') }}</button>
                  </div>
                </label>
              </div>
            </div>

            <div class="rounded-lg bg-muted/30 p-4 text-sm">
              <div class="flex justify-between py-1">
                <span class="text-muted-foreground">{{ t('loan.total_interest') }}</span>
                <span class="font-mono">{{ formatAmount(interestAmount) }} {{ selectedProduct?.asset_symbol || '' }}</span>
              </div>
              <div class="flex justify-between py-1">
                <span class="text-muted-foreground">{{ t('loan.repayment') }}</span>
                <span class="font-mono font-bold">{{ formatAmount(repaymentAmount) }} {{ selectedProduct?.asset_symbol || '' }}</span>
              </div>
            </div>

            <p v-if="validationMessage" class="text-sm text-red-500">{{ validationMessage }}</p>

            <button
              type="button"
              class="w-full rounded-lg bg-primary px-4 py-3 text-sm font-bold text-primary-foreground transition hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="submitDisabled"
              @click="handleApply"
            >
              {{ submitting ? t('loan.processing') : t('loan.apply_now') }}
            </button>
          </div>
        </aside>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Icon } from '@iconify/vue'
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'

import { applyLoan, fetchLoanProducts, localizedLoanName, type LoanProduct, type LoanType, type InterestCalculationMode } from '@/api/loan'
import { getWallets, type MemberWallet } from '@/api/asset'
import { useUserStore } from '@/stores/user'
import { formatNumber } from '@/utils/format'
import { estimateLoanInterest, estimateLoanRepayment, loanAmountRangeError, normalizeLoanAmountInput, parseLoanNumber } from '@/utils/loan'

const { t, locale } = useI18n()
const toast = useToast()
const userStore = useUserStore()

const products = ref<LoanProduct[]>([])
const walletAccounts = ref<MemberWallet[]>([])
const selectedProductId = ref<number | null>(null)
const amount = ref('')
const collateralAssetId = ref('')
const collateralAmount = ref('')
const loading = ref(false)
const submitting = ref(false)

const selectedProduct = computed(() => products.value.find((product) => product.id === selectedProductId.value) ?? products.value[0] ?? null)
const requiresCollateral = computed(() => selectedProduct.value?.loan_type === 'collateralized')
const collateralAmountNumber = computed(() => parseLoanNumber(collateralAmount.value))
const kycLevel = computed(() => Number(userStore.user?.kycLevel ?? userStore.user?.kyc_level ?? 0))
const kycEligible = computed(() => !selectedProduct.value || kycLevel.value >= selectedProduct.value.min_kyc_level)
const selectedCollateralWallet = computed(() => walletAccounts.value.find((wallet) => String(wallet.id) === collateralAssetId.value) ?? null)
const interestAmount = computed(() => estimateLoanInterest(amount.value, selectedProduct.value))
const repaymentAmount = computed(() => estimateLoanRepayment(amount.value, selectedProduct.value))
const amountPlaceholder = computed(() => selectedProduct.value ? amountRangeText(selectedProduct.value) : '0.00')
const validationMessage = computed(() => {
  const product = selectedProduct.value
  if (!userStore.isLoggedIn) return t('loan.login_required')
  if (!product) return t('loan.no_products')
  if (!amount.value.trim()) return ''
  const rangeError = loanAmountRangeError(amount.value, product)
  if (rangeError === 'invalid') return t('loan.invalid_amount')
  if (rangeError === 'below_min') return `${t('loan.min_amount_error')} ${formatAmount(product.min_amount)} ${product.asset_symbol}`
  if (rangeError === 'above_max') {
    return `${t('loan.max_amount_error')} ${formatAmount(product.max_amount)} ${product.asset_symbol}`
  }
  if (!kycEligible.value) return t('loan.kyc_required_desc', { level: product.min_kyc_level, current: kycLevel.value })
  if (requiresCollateral.value) {
    if (!collateralAssetId.value) return t('loan.select_collateral_asset')
    if (collateralAmountNumber.value === null || collateralAmountNumber.value <= 0) return t('loan.invalid_collateral_amount')
    if (selectedCollateralWallet.value && collateralAmountNumber.value > selectedCollateralWallet.value.balance) return t('loan.insufficient_collateral')
  }
  return ''
})
const submitDisabled = computed(() => Boolean(!selectedProduct.value || submitting.value))

onMounted(loadPage)

async function loadPage() {
  loading.value = true
  try {
    const [productResponse] = await Promise.all([
      fetchLoanProducts(),
      userStore.isLoggedIn ? userStore.loadProfile() : Promise.resolve(null),
    ])
    products.value = productResponse.data.data
    selectedProductId.value = products.value[0]?.id ?? null
    amount.value = products.value[0] ? defaultLoanAmount(products.value[0]) : ''
    if (userStore.isLoggedIn) {
      const walletResponse = await getWallets()
      walletAccounts.value = walletResponse.data.data
    }
  } catch (error) {
    toast.error(errorMessage(error, t('loan.load_failed')))
  } finally {
    loading.value = false
  }
}

function selectProduct(product: LoanProduct) {
  selectedProductId.value = product.id
  amount.value = defaultLoanAmount(product)
  collateralAssetId.value = ''
  collateralAmount.value = ''
}

function defaultLoanAmount(product: LoanProduct) {
  return normalizeLoanAmountInput(product.min_amount)
}

function productName(product: LoanProduct) {
  return localizedLoanName(product.name_json, product.name, String(locale.value || ''))
}

async function handleApply() {
  const product = selectedProduct.value
  if (!product || validationMessage.value) {
    toast.error(validationMessage.value || t('loan.apply_failed'))
    return
  }
  submitting.value = true
  try {
    await applyLoan({
      productId: product.id,
      amount: normalizeLoanAmountInput(amount.value),
      collateralAssetId: requiresCollateral.value ? Number(collateralAssetId.value) : undefined,
      collateralAmount: requiresCollateral.value ? normalizeLoanAmountInput(collateralAmount.value) : undefined,
    })
    toast.success(t('loan.apply_success'))
    amount.value = ''
    collateralAssetId.value = ''
    collateralAmount.value = ''
    await loadPage()
  } catch (error) {
    toast.error(errorMessage(error, t('loan.apply_failed')))
  } finally {
    submitting.value = false
  }
}

function setMaxCollateral() {
  if (!selectedCollateralWallet.value) return
  collateralAmount.value = String(selectedCollateralWallet.value.balance)
}

function loanTypeText(type: LoanType) {
  return t(type === 'collateralized' ? 'loan.type_collateralized' : 'loan.type_credit')
}

function interestModeText(mode: InterestCalculationMode) {
  return t(mode === 'actual_days' ? 'loan.interest_mode_actual_days' : 'loan.interest_mode_full_term')
}

function amountRangeText(product: LoanProduct) {
  const max = product.max_amount === null || product.max_amount === undefined ? t('loan.unlimited') : formatAmount(product.max_amount)
  return `${formatAmount(product.min_amount)} - ${max}`
}

function percentText(value: string | number) {
  return `${formatNumber(Number(value) * 100, 'percent')}`
}

function formatAmount(value: string | number | null | undefined) {
  return formatNumber(Number(value ?? 0), 'amount')
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>
