<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  Banknote,
  CheckCircle2,
  CircleAlert,
  Clock3,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
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
const applyDialog = ref<HTMLElement | null>(null)
const actionDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const amountNumber = computed(() => Number(amount.value || 0))
const collateralAmountNumber = computed(() => Number(collateralAmount.value || 0))
const selectedCollateral = computed(() => accounts.value.find((account) => account.assetId === collateralAssetId.value))
const dialogOpen = computed(() => Boolean(selected.value || pendingAction.value))
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

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
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
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('loan.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openApply(product: LoanProduct): void {
  if (!session.isAuthenticated) return
  selected.value = product
  amount.value = String(product.minAmount)
  collateralAssetId.value = accounts.value[0]?.assetId || 0
  collateralAmount.value = ''
  error.value = ''
  success.value = ''
}

function closeApply(): void {
  if (submitting.value) return
  selected.value = null
  error.value = ''
}

async function submitApplication(): Promise<void> {
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

function statusTone(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'repaid' || normalized === 'completed') return 'is-positive'
  if (normalized === 'overdue' || normalized === 'rejected' || normalized === 'cancelled' || normalized === 'canceled') return 'is-negative'
  return 'is-pending'
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    if (selected.value) closeApply()
    else closeOrderAction()
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
    const activeDialog = applyDialog.value || actionDialog.value
    activeDialog?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
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
  <main class="page page--plain loan-page">
    <PageHeader :title="t('loan.title')">
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

    <div class="page-content loan-content">
      <div v-if="error && !dialogOpen" class="loan-message loan-message--error" role="alert">
        <CircleAlert :size="18" />
        <span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load">
          <RefreshCw :size="17" />
        </button>
      </div>
      <div v-if="success" class="loan-message loan-message--success" role="status">
        <CheckCircle2 :size="18" />
        <span>{{ success }}</span>
      </div>

      <div v-if="loading" class="loan-loading" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <span>{{ t('loan.loading') }}</span>
      </div>

      <template v-else>
        <section class="loan-overview">
          <div class="loan-overview__icon"><Banknote :size="24" /></div>
          <div>
            <strong>{{ t('loan.bannerTitle') }}</strong>
            <p>{{ t('loan.bannerDescription') }}</p>
          </div>
          <ShieldCheck :size="20" />
        </section>

        <div v-if="products.length" class="loan-list">
          <button
            v-for="product in products"
            :key="product.id"
            class="loan-card"
            type="button"
            :disabled="!session.isAuthenticated"
            @click="openApply(product)"
          >
            <header>
              <AssetMark :symbol="product.assetSymbol" :size="38" />
              <span class="loan-kind">
                {{ product.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit') }}
              </span>
            </header>
            <div class="loan-card__body">
              <strong>{{ product.name }}</strong>
              <small>{{ product.assetSymbol }}</small>
            </div>
            <dl>
              <div>
                <dt>{{ t('loan.annualRate') }}</dt>
                <dd>{{ (product.interestRate * 100).toFixed(2) }}%</dd>
              </div>
              <div>
                <dt>{{ t('loan.term') }}</dt>
                <dd>{{ t('loan.termDays', { days: product.termDays }) }}</dd>
              </div>
            </dl>
          </button>
        </div>

        <div v-else class="loan-empty">
          <Banknote :size="24" />
          <span>{{ t('loan.noProducts') }}</span>
        </div>

        <LoginRequiredState
          v-if="!session.isAuthenticated"
          :description="t('loan.loginDescription')"
        />

        <section v-else class="loan-orders">
          <div class="section-heading">
            <span>{{ t('loan.myLoans') }}</span>
            <b>{{ orders.length }}</b>
          </div>

          <div v-if="orders.length" class="loan-order-list">
            <article v-for="order in orders" :key="order.id" class="loan-order">
              <header>
                <div>
                  <strong>{{ order.productName }}</strong>
                  <small>
                    {{ formatDateTime(order.createdAt) }} ·
                    <span :class="statusTone(order.status)">{{ order.status }}</span>
                  </small>
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
                  <dd v-if="order.dueAt">{{ formatDateTime(order.dueAt) }}</dd>
                  <dd v-else>{{ t('loan.termDays', { days: order.termDays }) }}</dd>
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
          </div>

          <div v-else class="loan-empty">
            <Clock3 :size="22" />
            <span>{{ t('loan.noOrders') }}</span>
          </div>
        </section>
      </template>
    </div>

    <div v-if="selected" class="loan-mask" @click.self="closeApply">
      <form
        ref="applyDialog"
        class="loan-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="loan-apply-title"
        @keydown="trapDialogFocus"
        @submit.prevent="submitApplication"
      >
        <header>
          <div>
            <strong id="loan-apply-title">{{ t('loan.applyTitle', { name: selected.name }) }}</strong>
            <small>
              {{ t('loan.minimum', {
                type: selected.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit'),
                amount: formatAmount(selected.minAmount),
                asset: selected.assetSymbol,
              }) }}
            </small>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="submitting"
            data-dialog-cancel
            @click="closeApply"
          >
            <X :size="21" />
          </button>
        </header>

        <label class="loan-field">
          <span>{{ t('loan.loanAmount') }}</span>
          <div>
            <input v-model="amount" class="numeric" inputmode="decimal" />
            <b>{{ selected.assetSymbol }}</b>
          </div>
        </label>

        <template v-if="selected.loanType === 'collateralized'">
          <label class="loan-field">
            <span>{{ t('loan.collateralAsset') }}</span>
            <select v-model="collateralAssetId">
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
          <label class="loan-field">
            <span>{{ t('loan.collateralAmount') }}</span>
            <div>
              <input v-model="collateralAmount" class="numeric" inputmode="decimal" />
              <b>{{ selectedCollateral?.symbol || '' }}</b>
            </div>
          </label>
        </template>

        <dl class="loan-terms">
          <div>
            <dt>{{ t('loan.term') }}</dt>
            <dd>{{ t('loan.termDays', { days: selected.termDays }) }}</dd>
          </div>
          <div>
            <dt>{{ t('loan.annualRate') }}</dt>
            <dd>{{ (selected.interestRate * 100).toFixed(2) }}%</dd>
          </div>
          <div>
            <dt>{{ t('loan.minimumKyc') }}</dt>
            <dd>{{ selected.minKycLevel }}</dd>
          </div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <button
          class="button button--primary button--full loan-submit"
          type="submit"
          :disabled="submitting"
          :aria-busy="submitting"
        >
          {{ submitting ? t('common.submitting') : t('loan.submit') }}
        </button>
      </form>
    </div>

    <div v-if="pendingAction" class="loan-mask" @click.self="closeOrderAction">
      <section
        ref="actionDialog"
        class="loan-dialog loan-action-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="loan-action-title"
        @keydown="trapDialogFocus"
      >
        <header>
          <div>
            <strong id="loan-action-title">{{ actionLabel(pendingAction) }}</strong>
            <small>
              {{ pendingAction.productName }} ·
              {{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}
            </small>
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

        <dl class="loan-terms">
          <div>
            <dt>{{ t('loan.loanAmount') }}</dt>
            <dd>{{ formatAmount(pendingAction.amount) }} {{ pendingAction.assetSymbol }}</dd>
          </div>
          <div>
            <dt>{{ t('loan.repaymentDue', { amount: formatAmount(pendingAction.repaymentAmount) }) }}</dt>
            <dd>{{ pendingAction.status }}</dd>
          </div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="dialog-actions">
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
            class="button"
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
  background: var(--surface);
  min-width: 0;
}

.loan-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
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

.loan-loading,
.loan-empty {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 132px;
  text-align: center;
}

.loan-overview {
  align-items: center;
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--accent) 9%, transparent), transparent 64%),
    var(--surface);
  border-block: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 11px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  min-height: 92px;
  min-width: 0;
  padding: 12px 4px;
}

.loan-overview__icon {
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  color: var(--accent);
  display: grid;
  height: 44px;
  place-items: center;
  width: 44px;
}

.loan-overview > div:nth-child(2) {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.loan-overview strong {
  font-size: 17px;
}

.loan-overview p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.loan-overview > svg {
  color: var(--positive);
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

.loan-card:not(:disabled):hover,
.loan-card:not(:disabled):focus-visible {
  border-color: var(--accent);
  box-shadow: inset 0 -3px 0 var(--accent);
}

.loan-card:disabled {
  cursor: default;
  opacity: .76;
}

.loan-card header {
  align-items: flex-start;
  display: flex;
  gap: 7px;
  justify-content: space-between;
  min-width: 0;
}

.loan-kind {
  background: color-mix(in srgb, var(--accent) 9%, var(--surface));
  border: 1px solid color-mix(in srgb, var(--accent) 25%, var(--line));
  color: var(--accent);
  font-size: 8px;
  font-weight: 800;
  line-height: 1.3;
  max-width: calc(100% - 45px);
  overflow-wrap: anywhere;
  padding: 4px 5px;
}

.loan-card__body {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.loan-card__body strong {
  font-size: 13px;
  overflow-wrap: anywhere;
}

.loan-card__body small {
  color: var(--muted);
  font-size: 9px;
}

.loan-card dl {
  border-top: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: auto 0 0;
  min-width: 0;
  padding-top: 9px;
}

.loan-card dl > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.loan-card dt,
.loan-card dd {
  font-size: 9px;
  margin: 0;
}

.loan-card dt {
  color: var(--muted);
}

.loan-card dd {
  font-variant-numeric: tabular-nums;
  font-weight: 800;
  overflow-wrap: anywhere;
}

.loan-card dl > div:first-child dd {
  color: var(--accent);
  font-size: 13px;
}

.loan-orders {
  border-top: 8px solid var(--soft);
  margin: 8px -20px 0;
  min-width: 0;
  padding: 0 20px;
}

.loan-orders .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 56px;
}

.loan-orders .section-heading b {
  color: var(--accent);
  font-size: 12px;
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
  background: rgb(5 10 16 / 62%);
  display: flex;
  inset: 0;
  justify-content: center;
  padding:
    max(16px, env(safe-area-inset-top))
    16px
    max(16px, env(safe-area-inset-bottom));
  position: fixed;
  z-index: 80;
}

.loan-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  box-shadow: 0 24px 60px rgb(5 10 16 / 28%);
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
  border-color: var(--focus, #1677ff);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus, #1677ff) 15%, transparent);
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
  .loan-content {
    padding-left: 14px;
    padding-right: 14px;
  }

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
