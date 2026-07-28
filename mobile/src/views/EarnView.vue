<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  CheckCircle2,
  CircleAlert,
  Landmark,
  LoaderCircle,
  PackageOpen,
  RefreshCw,
  ShieldCheck,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchEarnProducts, fetchEarnSubscriptions, redeemEarnSubscription, subscribeEarnProduct, type EarnProduct, type EarnSubscription } from '@/api/earn'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const { t } = useI18n()
const products = ref<EarnProduct[]>([])
const subscriptions = ref<EarnSubscription[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<EarnProduct | null>(null)
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const actionId = ref(0)
const error = ref('')
const success = ref('')
const subscribeDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const available = computed(() => accounts.value.find((account) => account.assetId === selected.value?.assetId)?.available || 0)
const amountNumber = computed(() => Number(amount.value || 0))
const dialogOpen = computed(() => Boolean(selected.value))
const canSubscribe = computed(() => {
  const product = selected.value
  return Boolean(product && Number.isFinite(amountNumber.value) && amountNumber.value >= product.minSubscribe && (!product.maxSubscribe || amountNumber.value <= product.maxSubscribe) && amountNumber.value <= available.value)
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProducts, nextSubscriptions, nextAccounts] = await Promise.all([fetchEarnProducts(), fetchEarnSubscriptions(), fetchWalletAccounts()])
    products.value = nextProducts
    subscriptions.value = nextSubscriptions
    accounts.value = nextAccounts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openSubscribe(product: EarnProduct): void {
  selected.value = product
  amount.value = String(product.minSubscribe)
  success.value = ''
  error.value = ''
}

function useMaximum(): void {
  amount.value = String(available.value)
}

function closeSubscribe(): void {
  if (submitting.value) return
  selected.value = null
  error.value = ''
}

async function subscribe(): Promise<void> {
  if (!selected.value || !canSubscribe.value) {
    error.value = t('earn.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await subscribeEarnProduct(selected.value.id, amountNumber.value)
    selected.value = null
    success.value = t('earn.subscribed')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.subscribeFailed'))
  } finally {
    submitting.value = false
  }
}

async function redeem(subscription: EarnSubscription): Promise<void> {
  actionId.value = subscription.id
  error.value = ''
  try {
    await redeemEarnSubscription(subscription.id)
    success.value = t('earn.redeemed')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.redeemFailed'))
  } finally {
    actionId.value = 0
  }
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeSubscribe()
    return
  }
  if (event.key !== 'Tab' || !subscribeDialog.value) return
  const focusable = Array.from(subscribeDialog.value.querySelectorAll<HTMLElement>(
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
    subscribeDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
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
  <main class="page page--plain earn-page">
    <PageHeader
      :back="true"
      :eyebrow="t('products.earn')"
      :subtitle="t('earn.bannerDescription')"
      :title="t('earn.title')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('earn.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content earn-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('earn.loginDescription')" />
      <template v-else>
        <div v-if="error && !dialogOpen" class="earn-message earn-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('earn.refresh')" @click="load">
            <RefreshCw :size="17" />
          </button>
        </div>
        <div v-if="success" class="earn-message earn-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ success }}</span>
        </div>
        <div v-if="loading" class="earn-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('earn.loading') }}</span>
        </div>
        <template v-else>
          <section class="earn-overview">
            <div class="earn-overview__icon"><Landmark :size="23" /></div>
            <div>
              <strong>{{ t('earn.bannerTitle') }}</strong>
              <p>{{ t('earn.bannerDescription') }}</p>
            </div>
            <ShieldCheck :size="20" />
          </section>

          <div v-if="products.length" class="earn-list">
            <button
              v-for="product in products"
              :key="product.id"
              class="earn-product"
              type="button"
              @click="openSubscribe(product)"
            >
              <AssetMark :symbol="product.assetSymbol" :size="40" />
              <div>
                <strong>{{ product.name || t('earn.defaultName', { asset: product.assetSymbol }) }}</strong>
                <small>{{ t('earn.term', { category: product.category, days: product.termDays }) }}</small>
              </div>
              <span>
                <b class="up numeric">{{ (product.aprRate * 100).toFixed(2) }}%</b>
                <small>{{ t('earn.estimatedApr') }}</small>
              </span>
            </button>
          </div>
          <div v-else class="earn-state earn-state--empty">
            <PackageOpen :size="23" />
            <span>{{ t('earn.emptyProducts') }}</span>
          </div>

          <section class="subscriptions">
            <div class="section-heading"><span>{{ t('earn.myHoldings') }}</span><b>{{ subscriptions.length }}</b></div>
            <article v-for="subscription in subscriptions" :key="subscription.id" class="subscription-row">
              <div>
                <strong>{{ t('earn.holdingSummary', { amount: formatAmount(subscription.amount), days: subscription.termDays }) }}</strong>
                <small>{{ t('earn.subscribedAt', { time: formatDateTime(subscription.subscribedAt) }) }}</small>
              </div>
              <span>
                <b>{{ subscription.status }}</b>
                <button
                  v-if="subscription.status === 'subscribed'"
                  class="button button--secondary"
                  type="button"
                  :disabled="actionId === subscription.id"
                  @click="redeem(subscription)"
                >
                  {{ actionId === subscription.id ? t('earn.redeeming') : t('earn.redeem') }}
                </button>
              </span>
            </article>
            <div v-if="!subscriptions.length" class="earn-state earn-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('earn.emptyHoldings') }}</span>
            </div>
          </section>
        </template>
      </template>
    </div>

    <div v-if="selected" class="earn-mask" @click.self="closeSubscribe">
      <form
        ref="subscribeDialog"
        class="earn-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="earn-subscribe-title"
        @keydown="trapDialogFocus"
        @submit.prevent="subscribe"
      >
        <header>
          <div>
            <strong id="earn-subscribe-title">{{ t('earn.subscribeTitle', { name: selected.name }) }}</strong>
            <small>{{ t('earn.subscribeSummary', { days: selected.termDays, apr: (selected.aprRate * 100).toFixed(2) }) }}</small>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="submitting"
            data-dialog-cancel
            @click="closeSubscribe"
          >
            <X :size="21" />
          </button>
        </header>
        <label class="earn-field">
          <span>{{ t('earn.amount') }}</span>
          <div>
            <input v-model="amount" class="numeric" inputmode="decimal" />
            <b>{{ selected.assetSymbol }}</b>
            <button type="button" @click="useMaximum">{{ t('earn.all') }}</button>
          </div>
        </label>
        <p class="earn-availability">{{ t('earn.availability', { available: formatAmount(available), asset: selected.assetSymbol, minimum: formatAmount(selected.minSubscribe) }) }}</p>
        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <button
          class="button button--primary button--full earn-submit"
          type="submit"
          :disabled="submitting"
          :aria-busy="submitting"
        >
          {{ submitting ? t('common.submitting') : t('earn.confirm') }}
        </button>
      </form>
    </div>
  </main>
</template>

<style scoped>
.earn-page {
  background: var(--surface);
  min-width: 0;
}

.earn-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.earn-message {
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

.earn-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.earn-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.earn-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.earn-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 148px;
  text-align: center;
}

.earn-state--empty {
  min-height: 112px;
}

.earn-overview {
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
  padding: 12px 4px;
}

.earn-overview__icon {
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  color: var(--accent);
  display: grid;
  height: 44px;
  place-items: center;
  width: 44px;
}

.earn-overview > div:nth-child(2) {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.earn-overview strong {
  font-size: 17px;
}

.earn-overview p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.earn-overview > svg {
  color: var(--positive);
}

.earn-list {
  border-block: 1px solid var(--line);
  display: grid;
}

.earn-product {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 40px minmax(0, 1fr) auto;
  min-height: 76px;
  padding: 10px 4px;
  text-align: left;
  width: 100%;
}

.earn-product:last-child {
  border-bottom: 0;
}

.earn-product:focus-visible,
.earn-product:hover {
  background: var(--surface-elevated);
  box-shadow: inset 3px 0 0 var(--accent);
}

.earn-product > div,
.earn-product > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.earn-product strong {
  font-size: 14px;
  overflow-wrap: anywhere;
}

.earn-product small {
  color: var(--muted);
  font-size: 10px;
}

.earn-product > span {
  text-align: right;
}

.earn-product > span b {
  font-size: 17px;
}

.subscriptions {
  border-top: 8px solid var(--soft);
  margin: 8px -20px 0;
  padding: 0 20px;
}

.subscriptions .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 56px;
}

.subscriptions .section-heading b {
  color: var(--accent);
  font-size: 12px;
}

.subscription-row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 72px;
  padding: 8px 0;
}

.subscription-row > div,
.subscription-row > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.subscription-row strong,
.subscription-row b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.subscription-row small {
  color: var(--muted);
  font-size: 10px;
}

.subscription-row > span {
  flex: 0 0 auto;
  justify-items: end;
}

.subscription-row .button {
  border-radius: 0;
  font-size: 11px;
  min-height: 44px;
  min-width: 88px;
  padding: 0 10px;
}

.earn-mask {
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

.earn-dialog {
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

.earn-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.earn-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.earn-dialog > header strong {
  font-size: 18px;
  overflow-wrap: anywhere;
}

.earn-dialog > header small,
.earn-availability {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.earn-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.earn-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.earn-field > span {
  color: var(--muted);
  font-size: 10px;
}

.earn-field > div {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  min-height: 44px;
}

.earn-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 20px;
  font-weight: 750;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.earn-field b {
  font-size: 12px;
  margin-right: 8px;
}

.earn-field button {
  background: transparent;
  color: var(--accent);
  font-size: 11px;
  font-weight: 800;
  min-height: 44px;
  padding: 0 2px 0 8px;
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

.earn-submit {
  border-radius: 0;
  min-height: 52px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .earn-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .subscriptions {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .earn-overview {
    grid-template-columns: 40px minmax(0, 1fr);
  }

  .earn-overview__icon {
    height: 40px;
    width: 40px;
  }

  .earn-overview > svg {
    display: none;
  }

  .earn-product {
    gap: 9px;
    grid-template-columns: 36px minmax(0, 1fr) auto;
  }

  .subscription-row {
    align-items: stretch;
    flex-direction: column;
  }

  .subscription-row > span {
    align-items: center;
    display: flex;
    justify-content: space-between;
  }
}
</style>
