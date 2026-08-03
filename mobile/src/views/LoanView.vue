<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  CheckCircle2,
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
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const router = useRouter()
const { t } = useI18n()
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
const riskNote = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const amountNumber = computed(() => Number(amount.value || 0))
const collateralAmountNumber = computed(() => Number(collateralAmount.value || 0))
const selectedCollateral = computed(() => accounts.value.find((account) => account.assetId === collateralAssetId.value))
const dialogOpen = computed(() => Boolean(pendingAction.value))
const hasProducts = computed(() => products.value.length > 0)
const loanWorkspaceState = computed(() => {
  if (error.value && !hasProducts.value) return 'error'
  if ((!productsReady.value || loading.value) && !hasProducts.value) return 'loading'
  return hasProducts.value ? 'ready' : 'empty'
})
const amountInvalid = computed(() => {
  const product = selected.value
  if (!product) return false
  return !Number.isFinite(amountNumber.value)
    || amountNumber.value < product.minAmount
    || Boolean(product.maxAmount && amountNumber.value > product.maxAmount)
})
const collateralInvalid = computed(() => {
  if (selected.value?.loanType !== 'collateralized') return false
  return !selectedCollateral.value
    || !Number.isFinite(collateralAmountNumber.value)
    || collateralAmountNumber.value <= 0
    || collateralAmountNumber.value > selectedCollateral.value.available
})
const canApply = computed(() => {
  const product = selected.value
  if (!product || !Number.isFinite(amountNumber.value) || amountNumber.value < product.minAmount) return false
  if (product.maxAmount && amountNumber.value > product.maxAmount) return false
  if (
    product.loanType === 'collateralized'
    && (
      !selectedCollateral.value
      || !Number.isFinite(collateralAmountNumber.value)
      || collateralAmountNumber.value <= 0
      || collateralAmountNumber.value > selectedCollateral.value.available
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
  const maximum = product.maxAmount || product.minAmount * 10
  return [...new Set([
    product.minAmount,
    Math.min(maximum, product.minAmount * 2),
    Math.min(maximum, product.minAmount * 5),
    maximum,
  ])].filter((value) => Number.isFinite(value) && value >= product.minAmount && value <= maximum)
})
const estimatedInterest = computed(() => {
  const product = selected.value
  if (!product || !Number.isFinite(amountNumber.value) || amountNumber.value <= 0) return 0
  return amountNumber.value * product.interestRate
})
const estimatedRepayment = computed(() => amountNumber.value + estimatedInterest.value)
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
  selected.value = product
  if (reset || !amount.value) amount.value = String(product.minAmount)
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
    await applyLoan({
      productId: selected.value.id,
      amount: amountNumber.value,
      collateralAssetId: selected.value.loanType === 'collateralized'
        ? collateralAssetId.value
        : undefined,
      collateralAmount: selected.value.loanType === 'collateralized'
        ? collateralAmountNumber.value
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

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeOrderAction()
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

watch(dialogOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    actionDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
    return
  }
  document.body.style.overflow = previousBodyOverflow
  await nextTick()
  returnFocus?.focus()
  returnFocus = null
})

onMounted(() => { void load() })

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
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

      <section class="loan-access-pencil" :class="{ 'loan-access-pencil--ready': session.isAuthenticated }">
        <div class="loan-access-pencil__summary">
          <span class="loan-access-pencil__icon"><ShieldCheck :size="20" /></span>
          <div>
            <strong>{{ session.isAuthenticated ? t('loan.accountReady') : t('loan.loginLimitTitle') }}</strong>
            <span>{{ session.isAuthenticated ? t('loan.accountReadyDescription') : t('loan.loginLimitDescription') }}</span>
          </div>
          <CheckCircle2 v-if="session.isAuthenticated" :size="18" />
        </div>
        <button v-if="!session.isAuthenticated" type="button" @click="openLogin">{{ t('loan.loginViewLimit') }}</button>
      </section>

      <nav class="pencil-segmented pencil-segmented--soft loan-categories" :aria-label="t('loan.productCategories')">
        <button type="button" :aria-pressed="productFilter === 'all'" @click="productFilter = 'all'">{{ t('common.all') }}</button>
        <button v-for="symbol in loanAssetFilters" :key="symbol" type="button" :aria-pressed="productFilter === symbol" @click="productFilter = symbol">{{ symbol }}</button>
      </nav>

      <div v-if="success" class="pencil-message pencil-message--success" role="status"><CheckCircle2 :size="18" /><span>{{ success }}</span></div>
      <div v-if="error && !dialogOpen" class="pencil-message pencil-message--error" role="alert">
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
            <button v-for="preset in amountPresets" :key="preset" type="button" :aria-pressed="amountNumber === preset" @click="amount = String(preset); error = ''; success = ''">{{ formatAmount(preset) }}</button>
          </div>

          <template v-if="selected.loanType === 'collateralized'">
            <label class="pencil-field">
              <span>{{ t('loan.collateralAsset') }}</span>
              <div class="pencil-field__shell">
                <select v-model="collateralAssetId" :disabled="!session.isAuthenticated">
                  <option v-if="!accounts.length" :value="0">{{ t('loan.loginDescription') }}</option>
                  <option v-for="account in accounts" :key="account.assetId" :value="account.assetId">{{ t('loan.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option>
                </select>
              </div>
            </label>
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
            <div><dt>{{ t('loan.estimatedInterest') }}</dt><dd class="pencil-numeric">{{ formatAmount(estimatedInterest) }} {{ selected.assetSymbol }}</dd></div>
            <div><dt>{{ t('loan.estimatedRepayment') }}</dt><dd class="pencil-numeric">{{ formatAmount(estimatedRepayment) }} {{ selected.assetSymbol }}</dd></div>
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
        @keydown="trapDialogFocus"
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

.loan-access-pencil {
  margin-top: 17px;
}

.loan-access-pencil__summary {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: 44px minmax(0, 1fr) 18px;
  height: 44px;
}

.loan-access-pencil__summary > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.loan-access-pencil__icon {
  align-items: center;
  background: var(--accent-soft);
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.loan-access-pencil strong {
  color: var(--ink);
  font-size: 12px;
  font-weight: 700;
  line-height: 17px;
}

.loan-access-pencil__summary > div > span {
  color: var(--muted);
  font-size: 9px;
  line-height: 14px;
}

.loan-access-pencil button {
  background: var(--ink);
  border-radius: 999px;
  color: var(--surface);
  font-size: 14px;
  font-weight: 700;
  height: 48px;
  margin-top: 16px;
  min-height: 48px;
  padding: 0 16px;
  width: 100%;
}

.loan-access-pencil:not(.loan-access-pencil--ready) > button {
  background: var(--accent);
  color: var(--on-accent);
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
}
</style>
