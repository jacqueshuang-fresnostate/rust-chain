<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Landmark,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  applyLoan,
  cancelLoanOrder,
  fetchLoanOrders,
  fetchLoanProducts,
  repayLoanOrder,
  type LoanOrder,
  type LoanProduct,
} from '@/api/loan'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import {
  decimalAdd,
  decimalCompare,
  decimalMinimum,
  decimalMultiply,
  decimalTextFromBoundary,
  decimalTextFromFiniteNumber,
  decimalWithinRange,
  formatDecimalText,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalText,
} from '@/core/decimal'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const router = useRouter()
const { locale, t } = useI18n()
const products = ref<LoanProduct[]>([])
const orders = ref<LoanOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<LoanProduct | null>(null)
const pendingAction = ref<LoanOrder | null>(null)
const amount = ref('')
const collateralAssetId = ref(0)
const collateralAmount = ref('')
const loading = ref(true)
const submitting = ref(false)
const actionId = ref(0)
const error = ref('')
const success = ref('')
const productFilter = ref('all')
const productsReady = ref(false)
const actionDialog = ref<HTMLElement | null>(null)
const collateralPickerOpen = ref(false)
const collateralPickerDialog = ref<HTMLElement | null>(null)
const riskNote = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const amountText = computed(() => positiveDecimalInput(amount.value))
const collateralAmountText = computed(() => positiveDecimalInput(collateralAmount.value))
const selectedCollateral = computed(() => accounts.value.find((account) => account.assetId === collateralAssetId.value))
const collateralAvailableText = computed(() => decimalTextFromBoundary(
  selectedCollateral.value?.availableText ?? selectedCollateral.value?.available,
  { allowNegative: false },
))
const dialogOpen = computed(() => Boolean(pendingAction.value))
const modalOpen = computed(() => dialogOpen.value || collateralPickerOpen.value)
const hasProducts = computed(() => products.value.length > 0)
const loanWorkspaceState = computed(() => {
  if (error.value && !hasProducts.value) return 'error'
  if ((!productsReady.value || loading.value) && !hasProducts.value) return 'loading'
  return hasProducts.value ? 'ready' : 'empty'
})
const amountInvalid = computed(() => {
  const product = selected.value
  if (!product) return false
  return !decimalWithinRange(amountText.value, {
    minimum: product.minAmountText ?? product.minAmount,
    maximum: product.maxAmountText ?? product.maxAmount,
  })
})
const collateralInvalid = computed(() => {
  if (selected.value?.loanType !== 'collateralized') return false
  return !selectedCollateral.value
    || !decimalWithinRange(collateralAmountText.value, { available: collateralAvailableText.value })
})
const canApply = computed(() => {
  const product = selected.value
  if (!product || !decimalWithinRange(amountText.value, {
    minimum: product.minAmountText ?? product.minAmount,
    maximum: product.maxAmountText ?? product.maxAmount,
  })) return false
  if (
    product.loanType === 'collateralized'
    && (
      !selectedCollateral.value
      || !decimalWithinRange(collateralAmountText.value, { available: collateralAvailableText.value })
    )
  ) return false
  return true
})
const loanAssetFilters = computed(() => {
  const heldSymbols = new Set(accounts.value.map((account) => account.symbol.toUpperCase()))
  return [...new Set(products.value.map((product) => product.assetSymbol.toUpperCase()).filter(Boolean))]
    .sort((left, right) => Number(heldSymbols.has(right)) - Number(heldSymbols.has(left)) || left.localeCompare(right))
    .slice(0, 4)
})
const visibleProducts = computed(() => productFilter.value === 'all'
  ? products.value
  : products.value.filter((product) => product.assetSymbol.toUpperCase() === productFilter.value))
const amountPresets = computed(() => {
  const product = selected.value
  if (!product) return []
  const minimum = product.minAmountText || decimalTextFromFiniteNumber(product.minAmount)
  const maximum = product.maxAmountText
    || (product.maxAmount ? decimalTextFromFiniteNumber(product.maxAmount) : decimalMultiply(minimum, normalizeDecimalText('10')))
  return [...new Set<DecimalText>([
    minimum,
    decimalMinimum(maximum, decimalMultiply(minimum, normalizeDecimalText('2'))) || minimum,
    decimalMinimum(maximum, decimalMultiply(minimum, normalizeDecimalText('5'))) || minimum,
    maximum,
  ])]
})
const estimatedInterest = computed<DecimalText>(() => {
  const product = selected.value
  if (!product || !amountText.value || !Number.isFinite(product.interestRate)) return normalizeDecimalText('0')
  return decimalMultiply(amountText.value, decimalTextFromFiniteNumber(product.interestRate))
})
const estimatedRepayment = computed(() => amountText.value
  ? decimalAdd(amountText.value, estimatedInterest.value)
  : normalizeDecimalText('0'))

function formatMoney(value: DecimalText): string {
  return formatDecimalText(value, locale.value === 'en' ? 'en-US' : 'zh-CN', { maximumFractionDigits: 18 })
}
async function load(): Promise<void> {
  loading.value = true
  productsReady.value = false
  error.value = ''
  try {
    const selectedProductId = selected.value?.id
    const productsPromise = fetchLoanProducts()
    if (session.isAuthenticated) {
      const [nextProducts, nextOrders, nextAccounts] = await Promise.all([
        productsPromise,
        fetchLoanOrders(),
        fetchWalletAccounts(),
      ])
      products.value = nextProducts
      orders.value = nextOrders
      accounts.value = nextAccounts
    } else {
      products.value = await productsPromise
      orders.value = []
      accounts.value = []
    }
    productsReady.value = true
    if (productFilter.value !== 'all' && !loanAssetFilters.value.includes(productFilter.value)) productFilter.value = 'all'
    const nextSelected = products.value.find((product) => product.id === selectedProductId) || null
    if (nextSelected) openApply(nextSelected, false)
    else selected.value = null
  } catch (reason) {
    products.value = []
    orders.value = []
    accounts.value = []
    selected.value = null
    error.value = apiErrorMessage(reason, t('loan.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openApply(product: LoanProduct, reset = true): void {
  collateralPickerOpen.value = false
  selected.value = product
  if (reset || !amount.value) amount.value = product.minAmountText || String(product.minAmount)
  collateralAssetId.value = accounts.value[0]?.assetId || 0
  collateralAmount.value = ''
  error.value = ''
  success.value = ''
}

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: '/products/loan' } })
}

function openRiskNote(): void {
  riskNote.value?.scrollIntoView({ behavior: 'smooth', block: 'center' })
}

function openCollateralPicker(): void {
  if (!session.isAuthenticated || pendingAction.value) return
  collateralPickerOpen.value = true
  error.value = ''
  success.value = ''
}

function closeCollateralPicker(): void {
  collateralPickerOpen.value = false
}

function selectCollateralAsset(account: WalletAccount): void {
  collateralAssetId.value = account.assetId
  error.value = ''
  success.value = ''
  closeCollateralPicker()
}

async function submitApplication(): Promise<void> {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!selected.value || !canApply.value) {
    error.value = t('loan.invalidApplication')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    const requestAmount = amountText.value
    const requestCollateral = selected.value.loanType === 'collateralized'
      ? collateralAmountText.value
      : undefined
    if (!requestAmount || (selected.value.loanType === 'collateralized' && !requestCollateral)) {
      throw new TypeError('invalid loan amount')
    }
    await applyLoan({
      productId: selected.value.id,
      amount: requestAmount,
      collateralAssetId: selected.value.loanType === 'collateralized'
        ? collateralAssetId.value
        : undefined,
      collateralAmount: selected.value.loanType === 'collateralized'
        ? requestCollateral || undefined
        : undefined,
    })
    selected.value = null
    success.value = t('loan.submitted')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('loan.submitFailed'))
  } finally {
    submitting.value = false
  }
}

function isRepayable(order: LoanOrder): boolean {
  const status = order.status.toLowerCase()
  return status === 'disbursed' || status === 'overdue'
}

function canActOnOrder(order: LoanOrder): boolean {
  return order.status.toLowerCase() === 'pending' || isRepayable(order)
}

function requestOrderAction(order: LoanOrder): void {
  if (collateralPickerOpen.value) return
  pendingAction.value = order
  error.value = ''
  success.value = ''
}

function closeOrderAction(): void {
  if (actionId.value) return
  pendingAction.value = null
  error.value = ''
}

async function confirmOrderAction(): Promise<void> {
  const order = pendingAction.value
  if (!order) return
  actionId.value = order.id
  error.value = ''
  try {
    if (order.status.toLowerCase() === 'pending') {
      await cancelLoanOrder(order.id)
      success.value = t('loan.canceled')
    } else {
      await repayLoanOrder(order.id)
      success.value = t('loan.repaid')
    }
    pendingAction.value = null
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(
      reason,
      order.status.toLowerCase() === 'pending' ? t('loan.cancelFailed') : t('loan.repayFailed'),
    )
  } finally {
    actionId.value = 0
  }
}

function actionLabel(order: LoanOrder): string {
  return order.status.toLowerCase() === 'pending' ? t('loan.cancel') : t('loan.repay')
}

function interestModeLabel(mode: string): string {
  const keys: Record<string, string> = {
    full_term: 'loan.interestModeFullTerm',
    actual_days: 'loan.interestModeActualDays',
  }
  const key = keys[mode.toLowerCase()]
  return key ? t(key) : mode || t('loan.interestModeUnavailable')
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'loan.statusPending',
    disbursed: 'loan.statusDisbursed',
    overdue: 'loan.statusOverdue',
    repaid: 'loan.statusRepaid',
    completed: 'loan.statusCompleted',
    rejected: 'loan.statusRejected',
    cancelled: 'loan.statusCancelled',
    canceled: 'loan.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function statusTone(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'repaid' || normalized === 'completed') return 'is-positive'
  if (normalized === 'overdue' || normalized === 'rejected' || normalized === 'cancelled' || normalized === 'canceled') return 'is-negative'
  return 'is-pending'
}

function trapDialogFocus(event: KeyboardEvent, close: () => void): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  if (event.key !== 'Tab') return
  const container = event.currentTarget instanceof HTMLElement ? event.currentTarget : null
  if (!container) return
  const focusable = Array.from(container.querySelectorAll<HTMLElement>(
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
  ))
  if (!focusable.length) return
  const first = focusable[0]
  const last = focusable.at(-1) || first
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

function handleActionDialogKeydown(event: KeyboardEvent): void {
  trapDialogFocus(event, closeOrderAction)
}

function handleCollateralPickerKeydown(event: KeyboardEvent): void {
  trapDialogFocus(event, closeCollateralPicker)
}

watch(modalOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    if (!modalOpen.value) return
    const initialFocus = collateralPickerOpen.value
      ? collateralPickerDialog.value?.querySelector<HTMLElement>('[data-collateral-current="true"]')
        || collateralPickerDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')
      : actionDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')
    initialFocus?.focus()
    return
  }
  if (modalOpen.value) return
  document.body.style.overflow = previousBodyOverflow
  await nextTick()
  returnFocus?.focus()
  returnFocus = null
})

onMounted(() => { void load() })

onBeforeUnmount(() => {
  if (modalOpen.value) document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main class="page page--plain pencil-page loan-pencil" data-loan-workspace="live" data-pencil-source="kIOBX yrsRy">
    <PageHeader :back="true" :pencil="true" :title="t('loan.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('loan.pencilRiskNote')" @click="openRiskNote"><ShieldCheck :size="18" /></button>
      </template>
    </PageHeader>

    <div class="pencil-content loan-pencil__content">
      <section class="loan-hero-pencil" :data-loan-state="loanWorkspaceState" :aria-busy="loading">
        <h1>{{ t('loan.heroTitle') }}</h1>
        <p>{{ t('loan.heroDescription') }}</p>
      </section>

      <button v-if="!session.isAuthenticated" class="loan-login-cta" type="button" @click="openLogin">{{ t('loan.loginViewLimit') }}</button>

      <nav class="pencil-segmented pencil-segmented--soft loan-categories" :aria-label="t('loan.productCategories')">
        <button type="button" :aria-pressed="productFilter === 'all'" @click="productFilter = 'all'">{{ t('common.all') }}</button>
        <button v-for="symbol in loanAssetFilters" :key="symbol" type="button" :aria-pressed="productFilter === symbol" @click="productFilter = symbol">{{ symbol }}</button>
      </nav>

      <div v-if="success" class="pencil-message pencil-message--success" role="status"><CheckCircle2 :size="18" /><span>{{ success }}</span></div>
      <div v-if="error && !modalOpen" class="pencil-message pencil-message--error" role="alert">
        <CircleAlert :size="18" /><span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="17" /></button>
      </div>
      <div v-else-if="loading && !hasProducts" class="pencil-state" role="status"><LoaderCircle :size="24" class="spin" /><span>{{ t('loan.loading') }}</span></div>

      <section v-else class="loan-products-pencil">
        <button v-for="product in visibleProducts" :key="product.id" class="loan-product-pencil" type="button" @click="openApply(product)">
          <AssetMark :symbol="product.assetSymbol" :size="40" />
          <span class="loan-product-pencil__copy">
            <strong>{{ product.name }}</strong>
            <small>{{ product.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit') }} · {{ t('loan.termDays', { days: product.termDays }) }}</small>
          </span>
          <span class="loan-product-pencil__rate">
            <b class="pencil-numeric">{{ (product.interestRate * 100).toFixed(2) }}%</b>
            <small>{{ t('loan.annualRate') }}</small>
          </span>
          <ChevronRight :size="17" />
          <dl>
            <div><dt>{{ t('loan.loanAmount') }}</dt><dd class="pencil-numeric">{{ formatAmount(product.minAmount) }}–{{ product.maxAmount ? formatAmount(product.maxAmount) : t('loan.noUpperLimit') }} {{ product.assetSymbol }}</dd></div>
            <div><dt>{{ t('loan.minimumKyc') }}</dt><dd>{{ product.minKycLevel }}</dd></div>
          </dl>
        </button>
        <div v-if="!visibleProducts.length" class="pencil-state loan-products-empty">
          <Landmark :size="25" /><strong>{{ t('loan.noProducts') }}</strong><span>{{ t('loan.bannerDescription') }}</span>
        </div>
      </section>

      <section v-if="selected" class="loan-application-pencil">
        <header>
          <div><span>{{ t('loan.applyTitle', { name: selected.name }) }}</span><strong>{{ selected.assetSymbol }}</strong></div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" @click="selected = null"><X :size="19" /></button>
        </header>
        <form @submit.prevent="submitApplication">
          <label class="pencil-field">
            <span>{{ t('loan.loanAmount') }}</span>
            <div class="pencil-field__shell" :class="{ 'is-invalid': amountInvalid }">
              <input v-model="amount" class="pencil-numeric" inputmode="decimal" :aria-invalid="amountInvalid" @input="error = ''; success = ''" />
              <b>{{ selected.assetSymbol }}</b>
            </div>
          </label>
          <div class="loan-presets">
            <button v-for="preset in amountPresets" :key="preset" type="button" :aria-pressed="Boolean(amountText && decimalCompare(amountText, preset) === 0)" @click="amount = preset; error = ''; success = ''">{{ formatMoney(preset) }}</button>
          </div>

          <template v-if="selected.loanType === 'collateralized'">
            <div class="pencil-field">
              <span>{{ t('loan.collateralAsset') }}</span>
              <button
                class="loan-collateral-trigger"
                type="button"
                :disabled="!session.isAuthenticated"
                aria-haspopup="dialog"
                :aria-expanded="collateralPickerOpen"
                aria-controls="loan-collateral-picker"
                @click="openCollateralPicker"
              >
                <AssetMark v-if="selectedCollateral" :symbol="selectedCollateral.symbol" :src="selectedCollateral.logoUrl" :size="34" />
                <span v-else class="loan-collateral-trigger__empty"><Landmark :size="18" /></span>
                <span class="loan-collateral-trigger__copy">
                  <strong>{{ selectedCollateral?.symbol || t('loan.noCollateralAssets') }}</strong>
                  <small v-if="selectedCollateral" class="pencil-numeric">{{ t('loan.availableBalance', { amount: formatAmount(selectedCollateral.available) }) }}</small>
                  <small v-else>{{ session.isAuthenticated ? t('loan.noCollateralAssets') : t('loan.loginDescription') }}</small>
                </span>
                <ChevronDown :size="18" />
              </button>
            </div>
            <label class="pencil-field">
              <span>{{ t('loan.collateralAmount') }}</span>
              <div class="pencil-field__shell" :class="{ 'is-invalid': collateralInvalid }">
                <input v-model="collateralAmount" class="pencil-numeric" inputmode="decimal" :aria-invalid="collateralInvalid" :disabled="!session.isAuthenticated" @input="error = ''; success = ''" />
                <b>{{ selectedCollateral?.symbol || '' }}</b>
              </div>
            </label>
          </template>

          <dl class="loan-estimate-pencil">
            <div><dt>{{ t('loan.term') }}</dt><dd>{{ t('loan.termDays', { days: selected.termDays }) }}</dd></div>
            <div><dt>{{ t('loan.estimatedInterest') }}</dt><dd class="pencil-numeric">{{ formatMoney(estimatedInterest) }} {{ selected.assetSymbol }}</dd></div>
            <div><dt>{{ t('loan.estimatedRepayment') }}</dt><dd class="pencil-numeric">{{ formatMoney(estimatedRepayment) }} {{ selected.assetSymbol }}</dd></div>
            <div><dt>{{ t('loan.interestModeUnavailable') }}</dt><dd>{{ interestModeLabel(selected.interestCalculationMode) }}</dd></div>
          </dl>
          <button class="pencil-primary pencil-primary--full" type="submit" :disabled="submitting || (session.isAuthenticated && !canApply)" :aria-busy="submitting">
            {{ submitting ? t('common.submitting') : session.isAuthenticated ? t('loan.submit') : t('auth.login') }}
          </button>
        </form>
      </section>

      <section v-if="session.isAuthenticated" class="pencil-section loan-orders-pencil">
        <div class="pencil-section__heading"><h2>{{ t('loan.myLoans') }}</h2><span class="pencil-pill">{{ orders.length }}</span></div>
        <div v-if="orders.length" class="pencil-list">
          <article v-for="order in orders" :key="order.id" class="pencil-row loan-order-pencil">
            <AssetMark :symbol="order.assetSymbol" :size="38" />
            <span class="pencil-row__copy">
              <strong>{{ order.productName }}</strong>
              <small>{{ formatDateTime(order.createdAt) }} · <i :class="statusTone(order.status)">{{ statusLabel(order.status) }}</i></small>
            </span>
            <span class="pencil-row__value">
              <strong class="pencil-numeric">{{ formatAmount(order.repaymentAmount) }} {{ order.assetSymbol }}</strong>
              <button v-if="canActOnOrder(order)" type="button" :disabled="actionId === order.id" @click="requestOrderAction(order)">
                {{ actionId === order.id ? t('loan.processing') : actionLabel(order) }}
              </button>
            </span>
          </article>
        </div>
        <div v-else class="pencil-state"><CheckCircle2 :size="23" /><span>{{ t('loan.noOrders') }}</span></div>
      </section>

      <div ref="riskNote" class="pencil-note loan-risk-note"><TriangleAlert :size="17" /><span>{{ t('loan.pencilRiskNote') }}</span></div>
    </div>

    <Teleport to="body">
      <div v-if="collateralPickerOpen" class="pencil-sheet-mask loan-collateral-mask" @click.self="closeCollateralPicker">
        <section
          id="loan-collateral-picker"
          ref="collateralPickerDialog"
          class="pencil-sheet loan-collateral-picker"
          role="dialog"
          aria-modal="true"
          aria-labelledby="loan-collateral-picker-title"
          @keydown="handleCollateralPickerKeydown"
        >
          <div class="pencil-sheet__handle" />
          <header>
            <h2 id="loan-collateral-picker-title">{{ t('loan.selectCollateralAsset') }}</h2>
            <button class="icon-button" type="button" :aria-label="t('common.close')" data-dialog-cancel @click="closeCollateralPicker"><X :size="20" /></button>
          </header>
          <div v-if="accounts.length" class="pencil-list loan-collateral-picker__list">
            <button
              v-for="account in accounts"
              :key="account.assetId"
              class="pencil-row loan-collateral-option"
              :class="{ 'is-selected': account.assetId === collateralAssetId }"
              type="button"
              :aria-pressed="account.assetId === collateralAssetId"
              :data-collateral-current="account.assetId === collateralAssetId"
              @click="selectCollateralAsset(account)"
            >
              <AssetMark :symbol="account.symbol" :src="account.logoUrl" :size="40" />
              <span class="pencil-row__copy">
                <strong>{{ account.symbol }}</strong>
                <small class="pencil-numeric">{{ t('loan.availableBalance', { amount: formatAmount(account.available) }) }}</small>
              </span>
              <span class="pencil-row__value"><Check v-if="account.assetId === collateralAssetId" :size="18" /></span>
            </button>
          </div>
          <div v-else class="pencil-state loan-collateral-picker__empty">
            <Landmark :size="24" />
            <strong>{{ t('loan.noCollateralAssets') }}</strong>
            <span>{{ t('loan.noCollateralAssetsDescription') }}</span>
          </div>
        </section>
      </div>
    </Teleport>

    <div v-if="pendingAction" class="confirmation-layer loan-mask" @click.self="closeOrderAction">
      <section
        ref="actionDialog"
        class="confirmation-sheet loan-dialog loan-action-dialog"
        :class="{ danger: pendingAction.status.toLowerCase() === 'pending' }"
        role="dialog"
        aria-modal="true"
        :aria-busy="Boolean(actionId)"
        aria-labelledby="loan-action-title"
        aria-describedby="loan-action-summary"
        tabindex="-1"
        @keydown="handleActionDialogKeydown"
      >
        <header>
          <span class="confirmation-icon"><CircleAlert v-if="pendingAction.status.toLowerCase() === 'pending'" :size="20" /><CheckCircle2 v-else :size="20" /></span>
          <div><strong id="loan-action-title">{{ actionLabel(pendingAction) }}</strong></div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="Boolean(actionId)" data-dialog-cancel @click="closeOrderAction"><X :size="21" /></button>
        </header>
        <p id="loan-action-summary">{{ pendingAction.productName }} · {{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}</p>
        <dl class="confirmation-detail loan-terms">
          <div><dt>{{ t('loan.loanAmount') }}</dt><dd>{{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}</dd></div>
          <div><dt>{{ t('loan.repaymentDue', { amount: formatAmount(pendingAction.repaymentAmount) }) }}</dt><dd>{{ statusLabel(pendingAction.status) }}</dd></div>
        </dl>
        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="confirmation-actions dialog-actions">
          <button type="button" class="button button--secondary" :disabled="Boolean(actionId)" @click="closeOrderAction">{{ t('common.cancel') }}</button>
          <button type="button" class="button confirmation-primary" :class="pendingAction.status.toLowerCase() === 'pending' ? 'button--danger' : 'button--primary'" :disabled="Boolean(actionId)" :aria-busy="Boolean(actionId)" @click="confirmOrderAction">
            {{ actionId ? t('loan.processing') : actionLabel(pendingAction) }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.loan-pencil__content {
  min-height: 474px;
  padding-top: 0;
}

.loan-hero-pencil {
  height: 72px;
  padding-top: 8px;
}

.loan-hero-pencil h1 {
  font-size: 22px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 32px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loan-hero-pencil > p {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  margin: 16px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loan-login-cta {
  background: var(--accent);
  border-radius: 999px;
  color: var(--on-accent);
  font-size: 14px;
  font-weight: 700;
  height: 48px;
  margin-top: 17px;
  min-height: 48px;
  padding: 0 16px;
  width: 100%;
}

.loan-login-cta:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.loan-categories {
  height: 30px;
  margin-top: 16px;
  min-height: 30px;
  overflow: visible;
}

.loan-categories button {
  height: 30px;
  min-height: 30px;
}

.loan-categories button::before {
  inset: -7px 0;
}

.loan-products-pencil {
  display: grid;
  gap: 0;
  padding-top: 16px;
}

.loan-product-pencil {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 40px minmax(0, 1fr) auto 18px;
  min-height: 98px;
  padding: 8px 0;
  text-align: left;
  width: 100%;
}

.loan-product-pencil:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.loan-product-pencil__copy,
.loan-product-pencil__rate {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.loan-product-pencil__copy strong {
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loan-product-pencil__copy small,
.loan-product-pencil__rate small {
  color: var(--muted);
  font-size: 9px;
}

.loan-product-pencil__rate {
  justify-items: end;
}

.loan-product-pencil__rate b {
  color: var(--positive);
  font-size: 16px;
}

.loan-product-pencil > dl {
  display: grid;
  gap: 6px;
  grid-column: 2 / 5;
  grid-template-columns: 1fr auto;
  margin: -3px 0 0;
}

.loan-product-pencil > dl > div {
  display: grid;
  gap: 3px;
}

.loan-product-pencil dt {
  color: var(--muted);
  font-size: 8px;
}

.loan-product-pencil dd {
  font-size: 9px;
  margin: 0;
}

.loan-products-empty {
  gap: 7px;
  height: 143px;
  min-height: 143px;
}

.loan-products-empty > svg {
  background: var(--accent-soft);
  border-radius: 50%;
  box-sizing: border-box;
  color: var(--positive);
  height: 56px;
  padding: 15px;
  width: 56px;
}

.loan-products-empty strong {
  color: var(--ink);
  font-size: 12px;
  font-weight: 600;
}

.loan-products-empty span {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.loan-application-pencil {
  background: var(--surface-2);
  border: 0;
  border-radius: 10px;
  box-shadow: none;
  margin-top: 16px;
  padding: 14px;
}

.loan-application-pencil > header {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.loan-application-pencil > header div {
  display: grid;
  gap: 3px;
}

.loan-application-pencil > header span {
  color: var(--muted);
  font-size: 10px;
}

.loan-application-pencil > header strong {
  font-size: 18px;
}

.loan-application-pencil form {
  display: grid;
  gap: 12px;
  margin-top: 12px;
}

.loan-collateral-trigger {
  align-items: center;
  background: var(--surface);
  border: 1px solid transparent;
  border-radius: 12px;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) 18px;
  min-height: 58px;
  padding: 8px 12px;
  text-align: left;
  width: 100%;
}

.loan-collateral-trigger:hover:not(:disabled) {
  background: var(--surface-3);
}

.loan-collateral-trigger:focus-visible {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 0;
}

.loan-collateral-trigger:disabled {
  cursor: not-allowed;
  opacity: 0.56;
}

.loan-collateral-trigger__empty {
  align-items: center;
  background: var(--surface-3);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 34px;
  justify-content: center;
  width: 34px;
}

.loan-collateral-trigger__copy {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.loan-collateral-trigger__copy strong,
.loan-collateral-trigger__copy small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loan-collateral-trigger__copy strong {
  font-size: 13px;
}

.loan-collateral-trigger__copy small {
  color: var(--muted);
  font-size: 10px;
}

.loan-presets {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, 1fr);
}

.loan-presets button {
  background: var(--surface);
  border-radius: 999px;
  color: var(--muted-strong);
  font-size: 9px;
  min-height: 32px;
  padding: 0 4px;
}

.loan-presets button[aria-pressed='true'] {
  background: var(--accent-soft);
  color: var(--positive);
}

.loan-estimate-pencil {
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 1fr;
  margin: 0;
}

.loan-estimate-pencil > div {
  display: grid;
  gap: 3px;
}

.loan-estimate-pencil dt {
  color: var(--muted);
  font-size: 8px;
}

.loan-estimate-pencil dd {
  font-size: 10px;
  margin: 0;
}

.loan-orders-pencil {
  margin-top: 16px;
}

.loan-order-pencil {
  grid-template-columns: 38px minmax(0, 1fr) auto;
}

.loan-order-pencil i {
  font-style: normal;
}

.loan-order-pencil .pencil-row__value strong {
  font-size: 10px;
}

.loan-order-pencil .pencil-row__value button {
  background: transparent;
  color: var(--positive);
  font-size: 10px;
  font-weight: 700;
  min-height: 44px;
  padding: 0;
}

.loan-risk-note {
  align-items: center;
  box-sizing: border-box;
  height: 36px;
  margin-top: 16px;
  min-height: 36px;
  overflow: hidden;
  padding: 0 10px;
}

.loan-risk-note > svg {
  flex: 0 0 auto;
}

.loan-risk-note > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loan-dialog {
  border-radius: 20px 20px 0 0;
  box-shadow: none;
}

.loan-collateral-mask {
  overscroll-behavior: contain;
}

.loan-collateral-picker {
  overflow-x: hidden;
}

.loan-collateral-picker .pencil-sheet__handle {
  margin-bottom: 12px;
}

.loan-collateral-picker > header {
  min-height: 52px;
}

.loan-collateral-picker :deep(.icon-button) {
  background: var(--surface-2);
  border: 0;
  border-radius: 50%;
  box-shadow: none;
  color: var(--ink);
  height: 44px;
  min-height: 44px;
  padding: 0;
  width: 44px;
}

.loan-collateral-picker__list {
  margin: 0 -8px;
}

.loan-collateral-option {
  border-radius: 12px;
  grid-template-columns: 40px minmax(0, 1fr) 24px;
  min-height: 68px;
  padding: 0 8px;
}

.loan-collateral-option.is-selected {
  background: var(--accent-soft);
}

.loan-collateral-option:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: -2px;
}

.loan-collateral-option .pencil-row__value {
  color: var(--positive);
  min-width: 24px;
}

.loan-collateral-picker__empty {
  gap: 8px;
  min-height: 180px;
}

.loan-collateral-picker__empty > svg {
  color: var(--muted);
}

.loan-collateral-picker__empty strong {
  color: var(--ink);
  font-size: 14px;
}

.loan-collateral-picker__empty span {
  color: var(--muted);
  font-size: 11px;
}

.loan-pencil :deep(.asset-mark) {
  border: 0;
  box-shadow: none;
}

@media (max-width: 340px) {
  .loan-hero-pencil h1 {
    font-size: 20px;
  }

  .loan-product-pencil {
    gap: 8px;
    grid-template-columns: 36px minmax(0, 1fr) auto 16px;
  }

  .loan-product-pencil__rate b {
    font-size: 14px;
  }

  .loan-presets {
    grid-template-columns: repeat(2, 1fr);
  }

  .loan-collateral-trigger {
    padding-inline: 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .loan-collateral-trigger {
    transition: none;
  }
}
</style>
