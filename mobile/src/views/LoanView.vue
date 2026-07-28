<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  CheckCircle2,
  CircleAlert,
  Fingerprint,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
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
const loading = ref(false)
const submitting = ref(false)
const actionId = ref(0)
const error = ref('')
const success = ref('')
const productsReady = ref(false)
const actionDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const amountNumber = computed(() => Number(amount.value || 0))
const collateralAmountNumber = computed(() => Number(collateralAmount.value || 0))
const selectedCollateral = computed(() => accounts.value.find((account) => account.assetId === collateralAssetId.value))
const dialogOpen = computed(() => Boolean(pendingAction.value))
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
const collateralProductCount = computed(() => products.value.filter((product) => product.loanType === 'collateralized').length)
const creditProductCount = computed(() => products.value.length - collateralProductCount.value)
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
const activeOrders = computed(() => orders.value.filter((order) => (
  ['pending', 'disbursed', 'overdue'].includes(order.status.toLowerCase())
)))
const historicalOrders = computed(() => orders.value.filter((order) => (
  !['pending', 'disbursed', 'overdue'].includes(order.status.toLowerCase())
)))

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
    const nextSelected = products.value.find((product) => product.id === selectedProductId) || products.value[0] || null
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

async function submitApplication(): Promise<void> {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/products/loan' } })
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
  <main class="secondary-view page page--plain page--prototype-grid loan-view" data-loan-workspace="live">
    <PageHeader
      :back="true"
      :eyebrow="t('loan.scene')"
      :subtitle="t('loan.context')"
      :title="t('loan.title')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('loan.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="secondary-content page-content loan-content">
      <section class="loan-page" data-loan-workflow="live">
        <section class="borrowing-overview" :aria-label="t('loan.bannerTitle')">
          <div>
            <span>{{ t('loan.bannerTitle') }}</span>
            <strong class="numeric">
              {{ productsReady && selected?.maxAmount ? formatAmount(selected.maxAmount) : '--' }}
              <small>{{ selected?.assetSymbol || '' }}</small>
            </strong>
            <p>{{ t('loan.bannerDescription') }}</p>
          </div>
          <dl>
            <div>
              <dt>{{ t('loan.collateralized') }}</dt>
              <dd class="numeric">{{ productsReady ? collateralProductCount : '--' }}</dd>
            </div>
            <div>
              <dt>{{ t('loan.credit') }}</dt>
              <dd class="numeric">{{ productsReady ? creditProductCount : '--' }}</dd>
            </div>
            <div>
              <dt>{{ t('loan.myLoans') }}</dt>
              <dd class="numeric">{{ session.isAuthenticated ? orders.length : '--' }}</dd>
            </div>
          </dl>
        </section>

        <h3 class="group-title">{{ t('loan.bannerTitle') }}</h3>
        <div class="product-choice-grid loan-list" :aria-label="t('loan.bannerTitle')">
          <button
            v-for="product in products"
            :key="product.id"
            class="loan-card"
            type="button"
            :class="{ active: selected?.id === product.id }"
            :aria-pressed="selected?.id === product.id"
            :data-loan-product="product.loanType"
            :disabled="loading"
            @click="openApply(product)"
          >
            <span class="product-kind">
              {{ product.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit') }}
            </span>
            <strong>{{ product.name }}</strong>
            <span>{{ t('loan.minimum', {
              type: product.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit'),
              amount: formatAmount(product.minAmount),
              asset: product.assetSymbol,
            }) }}</span>
            <span class="product-terms">
              <b>{{ (product.interestRate * 100).toFixed(2) }}%</b>
              <b>{{ t('loan.termDays', { days: product.termDays }) }}</b>
              <b>{{ product.maxAmount ? formatAmount(product.maxAmount) : '--' }} {{ product.assetSymbol }}</b>
            </span>
          </button>
          <button v-if="!products.length" class="loan-card loan-card--placeholder" type="button" disabled>
            <span class="product-kind">{{ t('loan.title') }}</span>
            <strong aria-live="polite">{{ loading ? t('loan.loading') : error || t('loan.noProducts') }}</strong>
            <span>{{ t('loan.bannerDescription') }}</span>
            <span class="product-terms"><b>--</b><b>--</b><b>--</b></span>
          </button>
          <button v-if="!products.length" class="loan-card loan-card--placeholder" type="button" disabled aria-hidden="true">
            <span class="product-kind">{{ t('loan.title') }}</span>
            <strong>--</strong>
            <span>{{ t('loan.bannerDescription') }}</span>
            <span class="product-terms"><b>--</b><b>--</b><b>--</b></span>
          </button>
        </div>

        <template v-if="selected">
          <div class="metric-grid loan-disclosures">
            <span>
              {{ t('loan.loanAmount') }}
              <b>{{ formatAmount(selected.minAmount) }}–{{ selected.maxAmount ? formatAmount(selected.maxAmount) : '--' }} {{ selected.assetSymbol }}</b>
            </span>
            <span>
              {{ t('loan.term') }}
              <b>{{ t('loan.termDays', { days: selected.termDays }) }}</b>
            </span>
            <span>
              {{ t('loan.minimumKyc') }}
              <b>{{ selected.minKycLevel }} · {{ (selected.interestRate * 100).toFixed(2) }}%</b>
            </span>
          </div>

          <div class="loan-requirement" :data-loan-requirement="selected.loanType">
            <ShieldCheck v-if="selected.loanType === 'collateralized'" :size="20" />
            <Fingerprint v-else :size="20" />
            <div>
              <strong>
                {{ selected.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit') }}
              </strong>
              <span>{{ session.isAuthenticated ? t('loan.bannerDescription') : t('loan.loginDescription') }}</span>
            </div>
          </div>

          <h3 class="group-title">{{ t('loan.applyTitle', { name: selected.name }) }}</h3>
          <form class="loan-application" @submit.prevent="submitApplication">
            <label
              class="field loan-field"
              :class="{ 'is-invalid': amountInvalid }"
              :data-field-state="amountInvalid ? 'invalid' : amount ? 'complete' : 'idle'"
            >
              <span>{{ t('loan.loanAmount') }}</span>
              <div>
                <input
                  v-model="amount"
                  class="numeric"
                  inputmode="decimal"
                  :aria-invalid="amountInvalid"
                  :disabled="loading"
                  @input="error = ''; success = ''"
                />
                <b>{{ selected.assetSymbol }}</b>
              </div>
            </label>

            <div class="amount-presets" :aria-label="t('loan.loanAmount')">
              <button
                v-for="preset in amountPresets"
                :key="preset"
                type="button"
                :aria-pressed="amountNumber === preset"
                :disabled="loading"
                @click="amount = String(preset); error = ''; success = ''"
              >
                {{ formatAmount(preset) }}
              </button>
            </div>

            <div v-if="selected.loanType === 'collateralized'" class="collateral-fields">
              <label class="field loan-field" :data-field-state="selectedCollateral ? 'complete' : 'idle'">
                <span>{{ t('loan.collateralAsset') }}</span>
                <select v-model="collateralAssetId" :disabled="loading || !session.isAuthenticated">
                  <option v-if="!accounts.length" :value="0">{{ t('loan.loginDescription') }}</option>
                  <option
                    v-for="account in accounts"
                    :key="account.assetId"
                    :value="account.assetId"
                  >
                    {{ t('loan.assetAvailable', {
                      asset: account.symbol,
                      amount: formatAmount(account.available),
                    }) }}
                  </option>
                </select>
              </label>
              <label
                class="field loan-field"
                :class="{ 'is-invalid': collateralInvalid }"
                :data-field-state="collateralInvalid ? 'invalid' : collateralAmount ? 'complete' : 'idle'"
              >
                <span>{{ t('loan.collateralAmount') }}</span>
                <div>
                  <input
                    v-model="collateralAmount"
                    class="numeric"
                    inputmode="decimal"
                    :aria-invalid="collateralInvalid"
                    :disabled="loading || !session.isAuthenticated"
                    @input="error = ''; success = ''"
                  />
                  <b>{{ selectedCollateral?.symbol || '--' }}</b>
                </div>
              </label>
            </div>

            <section class="loan-estimate" aria-live="polite">
              <div class="section-heading-row">
                <h3 class="group-title">{{ t('loan.repaymentDue', { amount: formatAmount(estimatedRepayment) }) }}</h3>
                <span>
                  {{ interestModeLabel(selected.interestCalculationMode) }}
                  · {{ t('loan.termDays', { days: selected.termDays }) }}
                </span>
              </div>
              <div class="loan-estimate-grid">
                <span>{{ t('loan.loanAmount') }}<b>{{ formatAmount(amountNumber) }} {{ selected.assetSymbol }}</b></span>
                <span>{{ t('loan.annualRate') }}<b>{{ (selected.interestRate * 100).toFixed(2) }}%</b></span>
                <span>{{ t('loan.estimatedInterest') }}<b>{{ formatAmount(estimatedInterest) }} {{ selected.assetSymbol }}</b></span>
                <span>{{ t('loan.estimatedRepayment') }}<b>{{ formatAmount(estimatedRepayment) }} {{ selected.assetSymbol }}</b></span>
              </div>
              <p>{{ t('loan.bannerDescription') }}</p>
            </section>

            <div class="loan-feedback" aria-live="polite">
              <div v-if="error && !dialogOpen" class="loan-message loan-message--error" role="alert">
                <CircleAlert :size="18" />
                <span>{{ error }}</span>
                <button type="button" :aria-label="t('common.retry')" @click="load">
                  <RefreshCw :size="17" />
                </button>
              </div>
              <div v-else-if="success" class="loan-message loan-message--success" role="status">
                <CheckCircle2 :size="18" />
                <span>{{ success }}</span>
              </div>
              <span v-else-if="loading">
                <LoaderCircle :size="15" class="spin" />
                {{ t('loan.loading') }}
              </span>
              <span v-else-if="!session.isAuthenticated">{{ t('loan.loginDescription') }}</span>
              <span v-else>{{ t('loan.bannerDescription') }}</span>
            </div>

            <button
              class="button button--primary button--full loan-submit"
              type="submit"
              :disabled="submitting || (session.isAuthenticated && !canApply)"
              :aria-busy="submitting"
            >
              {{ submitting ? t('common.submitting') : t('loan.submit') }}
            </button>
          </form>
        </template>

        <section class="loan-order-columns">
          <section>
            <header>
              <span>{{ t('loan.myLoans') }}</span>
              <b>{{ activeOrders.length }}</b>
            </header>
            <div class="lifecycle-list loan-order-list">
              <article v-for="order in activeOrders" :key="order.id" class="loan-order" :data-loan-status="order.status">
                <header>
                  <div>
                    <strong>{{ order.productName }}</strong>
                    <small>{{ formatDateTime(order.createdAt) }} · <span :class="statusTone(order.status)">{{ statusLabel(order.status) }}</span></small>
                  </div>
                  <AssetMark :symbol="order.assetSymbol" :size="32" />
                </header>
                <dl>
                  <div>
                    <dt>{{ t('loan.loanAmount') }}</dt>
                    <dd>{{ formatAmount(order.amount) }} {{ order.assetSymbol }}</dd>
                  </div>
                  <div>
                    <dt>{{ t('loan.repaymentDue', { amount: formatAmount(order.repaymentAmount) }) }}</dt>
                    <dd>{{ order.dueAt ? formatDateTime(order.dueAt) : t('loan.termDays', { days: order.termDays }) }}</dd>
                  </div>
                </dl>
                <button
                  v-if="canActOnOrder(order)"
                  class="button"
                  :class="order.status.toLowerCase() === 'pending' ? 'button--secondary' : 'button--primary'"
                  type="button"
                  :disabled="actionId === order.id"
                  @click="requestOrderAction(order)"
                >
                  {{ actionId === order.id ? t('loan.processing') : actionLabel(order) }}
                </button>
              </article>
              <p v-if="!activeOrders.length" class="loan-record-empty">
                {{ session.isAuthenticated ? t('loan.noOrders') : t('loan.loginDescription') }}
              </p>
            </div>
          </section>

          <section>
            <header>
              <span>{{ t('orders.history') }}</span>
              <b>{{ historicalOrders.length }}</b>
            </header>
            <div class="lifecycle-list loan-order-list">
              <article v-for="order in historicalOrders" :key="order.id" class="loan-order" :data-loan-status="order.status">
                <header>
                  <div>
                    <strong>{{ order.productName }}</strong>
                    <small>{{ formatDateTime(order.createdAt) }} · <span :class="statusTone(order.status)">{{ statusLabel(order.status) }}</span></small>
                  </div>
                  <AssetMark :symbol="order.assetSymbol" :size="32" />
                </header>
                <dl>
                  <div>
                    <dt>{{ t('loan.loanAmount') }}</dt>
                    <dd>{{ formatAmount(order.amount) }} {{ order.assetSymbol }}</dd>
                  </div>
                  <div>
                    <dt>{{ t('loan.repaymentDue', { amount: formatAmount(order.repaymentAmount) }) }}</dt>
                    <dd>{{ formatAmount(order.repaymentAmount) }} {{ order.assetSymbol }}</dd>
                  </div>
                </dl>
              </article>
              <p v-if="!historicalOrders.length" class="loan-record-empty">{{ t('loan.noOrders') }}</p>
            </div>
          </section>
        </section>
      </section>
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
          <span class="confirmation-icon">
            <CircleAlert v-if="pendingAction.status.toLowerCase() === 'pending'" :size="20" />
            <CheckCircle2 v-else :size="20" />
          </span>
          <div>
            <strong id="loan-action-title">{{ actionLabel(pendingAction) }}</strong>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="Boolean(actionId)"
            data-dialog-cancel
            @click="closeOrderAction"
          >
            <X :size="21" />
          </button>
        </header>

        <p id="loan-action-summary">
          {{ pendingAction.productName }} ·
          {{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}
        </p>

        <dl class="confirmation-detail loan-terms">
          <div>
            <dt>{{ t('loan.loanAmount') }}</dt>
            <dd>{{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}</dd>
          </div>
          <div>
            <dt>{{ t('loan.repaymentDue', { amount: formatAmount(pendingAction.repaymentAmount) }) }}</dt>
            <dd>{{ statusLabel(pendingAction.status) }}</dd>
          </div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="confirmation-actions dialog-actions">
          <button
            type="button"
            class="button button--secondary"
            :disabled="Boolean(actionId)"
            @click="closeOrderAction"
          >
            {{ t('common.cancel') }}
          </button>
          <button
            type="button"
            class="button confirmation-primary"
            :class="pendingAction.status.toLowerCase() === 'pending' ? 'button--danger' : 'button--primary'"
            :disabled="Boolean(actionId)"
            :aria-busy="Boolean(actionId)"
            @click="confirmOrderAction"
          >
            {{ actionId ? t('loan.processing') : actionLabel(pendingAction) }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.loan-page {
  display: grid;
  gap: 16px;
  min-width: 0;
}

.loan-message {
  align-items: center;
  border: 1px solid currentColor;
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 1.45;
  min-height: 52px;
  padding: 4px 5px 4px 11px;
}

.loan-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.loan-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.loan-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.loan-list {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-width: 0;
}

.loan-card {
  align-content: start;
  background: var(--surface);
  border: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 11px;
  min-height: 146px;
  min-width: 0;
  padding: 11px;
  text-align: left;
}

.loan-card:nth-child(3n + 1) { border-top: 3px solid var(--signal-green); }
.loan-card:nth-child(3n + 2) { border-top: 3px solid var(--signal-blue); }
.loan-card:nth-child(3n + 3) { border-top: 3px solid var(--signal-coral); }

.loan-card:not(:disabled):hover,
.loan-card:not(:disabled):focus-visible {
  border-color: var(--accent);
  box-shadow: inset 0 -3px 0 var(--accent);
}

.loan-card:disabled {
  cursor: default;
  opacity: .76;
}

.borrowing-overview {
  --overview-accent: var(--signal-green);
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--overview-accent) 8%, transparent), transparent 62%),
    var(--surface);
  border-bottom: 1px solid var(--line-strong);
  border-top: 3px solid var(--overview-accent);
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
  min-width: 0;
  overflow: hidden;
}

.borrowing-overview > div {
  align-content: center;
  border-right: 1px solid var(--line);
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 16px 12px 16px 0;
}

.borrowing-overview > div > span,
.borrowing-overview p,
.borrowing-overview dt {
  color: var(--muted);
  font-size: 9px;
}

.borrowing-overview strong {
  font-size: 24px;
}

.borrowing-overview strong small {
  color: var(--muted);
  font-size: 9px;
}

.borrowing-overview p {
  line-height: 1.45;
  margin: 0;
}

.borrowing-overview dl {
  align-content: center;
  display: grid;
  gap: 7px;
  margin: 0;
  min-width: 0;
  padding: 12px 0 12px 12px;
}

.borrowing-overview dl > div {
  display: flex;
  gap: 8px;
  justify-content: space-between;
  min-width: 0;
}

.borrowing-overview dd {
  font-size: 10px;
  font-weight: 700;
  margin: 0;
}

.product-choice-grid button {
  align-content: start;
  min-height: 150px;
}

.product-choice-grid button.active {
  background: color-mix(in srgb, var(--accent) 8%, var(--surface));
  border-color: var(--accent);
  box-shadow: inset 0 -3px 0 var(--accent);
}

.product-choice-grid .product-kind {
  color: var(--accent);
  font-size: 9px;
  font-weight: 700;
}

.product-choice-grid > button > span:not(.product-kind, .product-terms) {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.45;
}

.product-choice-grid .product-terms {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: auto;
}

.product-choice-grid .product-terms b {
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--muted-strong);
  font-size: 8px;
  padding: 4px 6px;
}

.loan-card--placeholder {
  opacity: .62;
}

.loan-disclosures {
  border-left: 1px solid var(--line);
  border-top: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.loan-disclosures span {
  border-bottom: 1px solid var(--line);
  border-right: 1px solid var(--line);
  color: var(--muted);
  display: grid;
  font-size: 9px;
  gap: 5px;
  min-width: 0;
  padding: 10px 8px;
}

.loan-disclosures b {
  color: var(--ink);
  font-size: 10px;
  overflow-wrap: anywhere;
}

.loan-requirement {
  border-block: 1px solid var(--line);
  border-left: 2px solid var(--accent);
  color: var(--accent);
  display: grid;
  gap: 10px;
  grid-template-columns: 24px minmax(0, 1fr);
  min-width: 0;
  padding: 13px 12px;
}

.loan-requirement[data-loan-requirement='collateralized'] {
  border-left-color: var(--signal-coral);
  color: var(--signal-coral);
}

.loan-requirement > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.loan-requirement strong {
  color: var(--ink);
  font-size: 12px;
}

.loan-requirement span {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.5;
}

.loan-application,
.collateral-fields {
  display: grid;
  gap: 12px;
}

.amount-presets {
  display: grid;
  gap: 4px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.amount-presets button {
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--muted);
  font-size: 9px;
  min-height: 44px;
  min-width: 0;
}

.amount-presets button[aria-pressed='true'] {
  background: color-mix(in srgb, var(--accent) 8%, var(--soft));
  border-color: var(--accent);
  color: var(--ink);
}

.loan-estimate {
  border-bottom: 1px solid var(--line-strong);
  display: grid;
  gap: 10px;
  padding-block: 4px 14px;
}

.loan-estimate .section-heading-row > span {
  color: var(--muted);
  font-size: 9px;
}

.loan-estimate-grid {
  border-left: 1px solid var(--line);
  border-top: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.loan-estimate-grid span {
  border-bottom: 1px solid var(--line);
  border-right: 1px solid var(--line);
  color: var(--muted);
  display: grid;
  font-size: 9px;
  gap: 5px;
  min-width: 0;
  padding: 11px;
}

.loan-estimate-grid b {
  color: var(--ink);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.loan-estimate > p {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.5;
  margin: 0;
}

.loan-feedback {
  align-content: start;
  display: grid;
  min-height: 76px;
}

.loan-feedback > span {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 6px;
}

.loan-order-columns {
  display: grid;
  gap: 16px;
}

.loan-order-columns > section {
  min-width: 0;
}

.loan-order-columns > section > header {
  align-items: center;
  border-bottom: 1px solid var(--line-strong);
  display: flex;
  justify-content: space-between;
  min-height: 40px;
  padding-inline: 2px;
}

.loan-order-columns > section > header span,
.loan-order-columns > section > header b {
  font-size: 10px;
}

.loan-order-columns > section > header b {
  color: var(--accent);
}

.lifecycle-list article {
  background: transparent;
  border-width: 0 0 1px;
}

.loan-record-empty {
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  display: grid;
  font-size: 10px;
  margin: 0;
  min-height: 88px;
  padding: 14px;
  place-items: center;
  text-align: center;
}

.loan-order {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 11px;
  min-width: 0;
  padding: 14px 0;
}

.loan-order > header {
  align-items: center;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.loan-order > header > div {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.loan-order strong {
  font-size: 13px;
  overflow-wrap: anywhere;
}

.loan-order small {
  color: var(--muted);
  font-size: 9px;
}

.loan-order small .is-positive {
  color: var(--positive);
}

.loan-order small .is-negative {
  color: var(--negative);
}

.loan-order small .is-pending {
  color: var(--accent);
}

.loan-order > dl {
  border-block: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: 0;
}

.loan-order > dl > div {
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 10px 8px;
}

.loan-order > dl > div:first-child {
  border-right: 1px solid var(--line);
}

.loan-order dt,
.loan-order dd {
  font-size: 9px;
  margin: 0;
  overflow-wrap: anywhere;
}

.loan-order dt {
  color: var(--muted);
}

.loan-order dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
}

.loan-order > .button {
  justify-self: end;
  min-height: 44px;
  min-width: 112px;
}

.loan-mask {
  align-items: flex-end;
  background: var(--overlay);
  display: flex;
  inset: 0;
  justify-content: center;
  padding:
    max(16px, env(safe-area-inset-top))
    16px
    max(16px, env(safe-area-inset-bottom));
  position: fixed;
  z-index: var(--layer-overlay);
}

.loan-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 14px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.loan-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.loan-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.loan-dialog > header strong {
  font-size: 18px;
  overflow-wrap: anywhere;
}

.loan-dialog > header small {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.loan-field {
  background: var(--soft);
  border: 1px solid var(--line);
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.loan-field:focus-within {
  background: var(--surface);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.loan-field.is-invalid {
  border-color: var(--negative);
}

.loan-field.is-invalid:focus-within {
  border-color: var(--negative);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--negative) 16%, transparent);
}

.loan-field > span {
  color: var(--muted);
  font-size: 10px;
}

.loan-field > div {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 38px;
}

.loan-field input,
.loan-field select {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-height: 38px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.loan-field input {
  font-size: 20px;
  font-weight: 750;
}

.loan-field select {
  font-size: 13px;
}

.loan-field b {
  font-size: 12px;
}

.loan-terms {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.loan-terms > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.loan-terms > div:last-child {
  border-bottom: 0;
}

.loan-terms dt,
.loan-terms dd {
  font-size: 12px;
  margin: 0;
}

.loan-terms dt {
  color: var(--muted);
}

.loan-terms dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  text-align: right;
}

.dialog-feedback {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  padding: 8px 10px;
}

.loan-submit {
  border-radius: 0;
  min-height: 52px;
}

.loan-action-dialog {
  border-top-color: var(--negative);
}

.dialog-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
}

.dialog-actions .button {
  min-height: 48px;
  padding-inline: 10px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .loan-orders {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .loan-list {
    grid-template-columns: 1fr;
  }

  .loan-card {
    min-height: 132px;
  }

  .loan-order > dl {
    grid-template-columns: 1fr;
  }

  .loan-order > dl > div:first-child {
    border-bottom: 1px solid var(--line);
    border-right: 0;
  }

  .dialog-actions {
    grid-template-columns: 1fr;
  }
}
</style>
