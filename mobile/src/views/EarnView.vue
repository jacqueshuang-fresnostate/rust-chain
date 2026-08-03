<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  CheckCircle2,
  CircleAlert,
  History,
  Landmark,
  LoaderCircle,
  PackageOpen,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchEarnProducts, fetchEarnSubscriptions, redeemEarnSubscription, subscribeEarnProduct, type EarnProduct, type EarnSubscription } from '@/api/earn'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const router = useRouter()
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
const activeCategory = ref('all')
const subscribeDialog = ref<HTMLElement | null>(null)
const holdingsSection = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const available = computed(() => accounts.value.find((account) => account.assetId === selected.value?.assetId)?.available || 0)
const amountNumber = computed(() => Number(amount.value || 0))
const dialogOpen = computed(() => Boolean(selected.value))
const canSubscribe = computed(() => {
  const product = selected.value
  return Boolean(product && Number.isFinite(amountNumber.value) && amountNumber.value >= product.minSubscribe && (!product.maxSubscribe || amountNumber.value <= product.maxSubscribe) && amountNumber.value <= available.value)
})
const categories = computed(() => [
  { value: 'all', label: t('earn.all') },
  ...[...new Set(products.value.map((product) => product.category).filter(Boolean))]
    .slice(0, 4)
    .map((value) => ({ value, label: value })),
])
const visibleProducts = computed(() => activeCategory.value === 'all'
  ? products.value
  : products.value.filter((product) => product.category === activeCategory.value))

async function load(): Promise<void> {
  if (!session.isAuthenticated) {
    products.value = []
    subscriptions.value = []
    accounts.value = []
    activeCategory.value = 'all'
    error.value = ''
    loading.value = false
    return
  }
  loading.value = true
  error.value = ''
  try {
    const productPromise = fetchEarnProducts()
    const [nextProducts, nextSubscriptions, nextAccounts] = await Promise.all([productPromise, fetchEarnSubscriptions(), fetchWalletAccounts()])
    products.value = nextProducts
    subscriptions.value = nextSubscriptions
    accounts.value = nextAccounts
  } catch (reason) {
    session.sync()
    if (!session.isAuthenticated) {
      products.value = []
      subscriptions.value = []
      accounts.value = []
      error.value = ''
    } else {
      error.value = apiErrorMessage(reason, t('earn.loadFailed'))
    }
  } finally {
    loading.value = false
  }
}

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: '/products/earn' } })
}

function openSubscribe(product: EarnProduct): void {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  selected.value = product
  amount.value = String(product.minSubscribe)
  success.value = ''
  error.value = ''
}

function useMaximum(): void {
  amount.value = String(available.value)
}

function openHoldings(): void {
  holdingsSection.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
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
  <main class="page page--plain pencil-page earn-pencil" data-pencil-source="zIzOm tCHZ9">
    <PageHeader :back="true" :pencil="true" :title="t('earn.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('earn.myHoldings')" @click="openHoldings"><History :size="18" /></button>
      </template>
    </PageHeader>

    <div class="pencil-content earn-pencil__content">
      <section class="earn-hero-pencil">
        <h1>{{ t('earn.heroTitle') }}</h1>
        <p>{{ t('earn.heroDescription') }}</p>
      </section>

      <nav class="pencil-segmented earn-categories" :aria-label="t('earn.productCategories')">
        <button v-for="category in categories" :key="category.value" type="button" :aria-pressed="activeCategory === category.value" @click="activeCategory = category.value">
          {{ category.label }}
        </button>
      </nav>

      <div v-if="!session.isAuthenticated" ref="holdingsSection" class="earn-guest-state">
        <div class="pencil-state earn-guest-state__summary">
          <span class="earn-followup-state__icon"><Landmark :size="22" /></span>
          <strong>{{ t('common.loginRequiredTitle') }}</strong>
          <span>{{ t('earn.loginDescription') }}</span>
        </div>
        <button class="pencil-primary pencil-primary--full" type="button" @click="openLogin">{{ t('auth.login') }}</button>
      </div>
      <div v-else-if="error && !dialogOpen" class="pencil-message pencil-message--error" role="alert">
        <CircleAlert :size="18" /><span>{{ error }}</span>
        <button type="button" :aria-label="t('earn.refresh')" @click="load"><RefreshCw :size="17" /></button>
      </div>
      <div v-else-if="loading" class="pencil-state earn-followup-state" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <strong>{{ t('earn.moreProductsLoading') }}</strong>
        <span>{{ t('earn.moreProductsLoadingDescription') }}</span>
      </div>

      <template v-else>
        <div v-if="success" class="pencil-message pencil-message--success" role="status"><CheckCircle2 :size="18" /><span>{{ success }}</span></div>
        <div v-if="visibleProducts.length" class="earn-list-pencil">
          <button v-for="product in visibleProducts" :key="product.id" class="earn-product-pencil" type="button" @click="openSubscribe(product)">
            <header>
              <strong>{{ product.name || t('earn.defaultName', { asset: product.assetSymbol }) }}</strong>
              <b class="pencil-pill">{{ product.termDays ? t('earn.termDays', { days: product.termDays }) : t('earn.flexible') }}</b>
            </header>
            <dl>
              <div><dt>{{ t('earn.estimatedApr') }}</dt><dd class="up pencil-numeric">{{ (product.aprRate * 100).toFixed(2) }}%</dd></div>
              <div><dt>{{ t('earn.minimumLabel') }}</dt><dd class="pencil-numeric">{{ formatAmount(product.minSubscribe) }} {{ product.assetSymbol }}</dd></div>
              <div><dt>{{ t('earn.riskLabel') }}</dt><dd>{{ t('earn.platformRules') }}</dd></div>
            </dl>
            <p>{{ product.category }} · {{ t('earn.bannerDescription') }}</p>
            <span class="earn-product-pencil__action">{{ t('earn.viewAvailableProducts') }}</span>
          </button>
        </div>
        <div v-else ref="holdingsSection" class="pencil-state earn-followup-state">
          <PackageOpen :size="23" />
          <strong>{{ t('earn.emptyProducts') }}</strong>
          <span>{{ t('earn.moreProductsLoadingDescription') }}</span>
        </div>

        <section v-if="session.isAuthenticated && subscriptions.length" ref="holdingsSection" class="pencil-section earn-holdings-pencil">
          <div class="pencil-section__heading"><h2>{{ t('earn.myHoldings') }}</h2><span class="pencil-pill">{{ subscriptions.length }}</span></div>
          <div class="pencil-list">
            <article v-for="subscription in subscriptions" :key="subscription.id" class="pencil-row earn-holding-row">
              <span class="pencil-row__icon"><Landmark :size="18" /></span>
              <span class="pencil-row__copy">
                <strong>{{ t('earn.holdingSummary', { amount: formatAmount(subscription.amount), days: subscription.termDays }) }}</strong>
                <small>{{ t('earn.subscribedAt', { time: formatDateTime(subscription.subscribedAt) }) }}</small>
              </span>
              <span class="pencil-row__value">
                <small>{{ subscription.status }}</small>
                <button v-if="subscription.status === 'subscribed'" type="button" :disabled="actionId === subscription.id" @click="redeem(subscription)">
                  {{ actionId === subscription.id ? t('earn.redeeming') : t('earn.redeem') }}
                </button>
              </span>
            </article>
          </div>
        </section>
        <div v-else-if="visibleProducts.length" ref="holdingsSection" class="pencil-state earn-followup-state">
          <span class="earn-followup-state__icon"><Landmark :size="22" /></span>
          <strong>{{ t('earn.moreProductsLoading') }}</strong>
          <span>{{ t('earn.moreProductsLoadingDescription') }}</span>
        </div>
      </template>
    </div>

    <div v-if="selected" class="earn-mask" @click.self="closeSubscribe">
      <form ref="subscribeDialog" class="earn-dialog" role="dialog" aria-modal="true" aria-labelledby="earn-subscribe-title" @keydown="trapDialogFocus" @submit.prevent="subscribe">
        <header>
          <div><strong id="earn-subscribe-title">{{ t('earn.subscribeTitle', { name: selected.name }) }}</strong><small>{{ t('earn.subscribeSummary', { days: selected.termDays, apr: (selected.aprRate * 100).toFixed(2) }) }}</small></div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="submitting" data-dialog-cancel @click="closeSubscribe"><X :size="21" /></button>
        </header>
        <label class="pencil-field">
          <span>{{ t('earn.amount') }}</span>
          <div class="pencil-field__shell">
            <input v-model="amount" class="pencil-numeric" inputmode="decimal" />
            <b>{{ selected.assetSymbol }}</b>
            <button type="button" @click="useMaximum">{{ t('earn.all') }}</button>
          </div>
        </label>
        <p class="earn-availability">{{ t('earn.availability', { available: formatAmount(available), asset: selected.assetSymbol, minimum: formatAmount(selected.minSubscribe) }) }}</p>
        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <button class="pencil-primary pencil-primary--full" type="submit" :disabled="submitting" :aria-busy="submitting">{{ submitting ? t('common.submitting') : t('earn.confirm') }}</button>
      </form>
    </div>
  </main>
</template>

<style scoped>
.earn-pencil__content {
  min-height: 474px;
  padding-top: 0;
}

.earn-hero-pencil {
  height: 72px;
  padding-top: 8px;
}

.earn-hero-pencil h1 {
  font-size: 22px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 32px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.earn-hero-pencil p {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  margin: 16px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.earn-categories {
  height: 26px;
  margin-top: 17px;
  min-height: 26px;
  overflow: visible;
}

.earn-categories button {
  border-bottom: 0;
  margin-top: -9px;
  min-height: 44px;
  padding: 0 0 9px;
}

.earn-categories button::after {
  background: transparent;
  bottom: 9px;
  content: '';
  height: 2px;
  left: 2px;
  position: absolute;
  width: 18px;
}

.earn-categories button[aria-pressed='true']::after {
  background: var(--accent);
}

.earn-list-pencil {
  display: grid;
  gap: 16px;
  padding-top: 16px;
}

.earn-product-pencil {
  background: transparent;
  border: 0;
  border-radius: 0;
  color: var(--ink);
  display: grid;
  height: 172px;
  padding: 0;
  text-align: left;
  width: 100%;
}

.earn-product-pencil:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 3px;
}

.earn-product-pencil > header {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 24px;
}

.earn-product-pencil > header strong {
  font-size: 14px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.earn-product-pencil dl {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 36px;
  margin: 14px 0 0;
}

.earn-product-pencil dl > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.earn-product-pencil dt {
  color: var(--muted);
  font-size: 9px;
  line-height: 12px;
}

.earn-product-pencil dd {
  font-size: 11px;
  line-height: 16px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.earn-product-pencil dl > div:first-child dd {
  color: var(--positive);
  font-size: 14px;
  font-weight: 700;
}

.earn-product-pencil > p {
  color: var(--muted);
  font-size: 10px;
  line-height: 16px;
  margin: 18px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.earn-product-pencil__action {
  align-items: center;
  background: var(--accent);
  border-radius: 999px;
  color: var(--on-accent);
  display: flex;
  font-size: 14px;
  font-weight: 700;
  height: 48px;
  justify-content: center;
  margin-top: 16px;
  width: 100%;
}

.earn-followup-state {
  gap: 6px;
  height: 135px;
  min-height: 135px;
}

.earn-guest-state {
  display: grid;
  gap: 16px;
  grid-template-rows: 108px 48px;
  height: 172px;
  margin-top: 16px;
}

.earn-guest-state__summary {
  gap: 5px;
  height: 108px;
  min-height: 108px;
}

.earn-guest-state__summary strong {
  color: var(--ink);
  font-size: 12px;
  font-weight: 600;
}

.earn-guest-state__summary > span:not(.earn-followup-state__icon) {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.earn-list-pencil + .earn-followup-state,
.earn-categories + .pencil-message + .earn-followup-state,
.earn-categories + .earn-followup-state,
.earn-holdings-pencil {
  margin-top: 16px;
}

.earn-followup-state__icon {
  align-items: center;
  background: var(--accent-soft);
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}

.earn-followup-state strong {
  color: var(--ink);
  font-size: 12px;
  font-weight: 600;
}

.earn-followup-state > span:not(.earn-followup-state__icon) {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.earn-holdings-pencil {
  scroll-margin-top: 60px;
}

.earn-holding-row {
  grid-template-columns: 40px minmax(0, 1fr) auto;
}

.earn-holding-row .pencil-row__value button {
  background: transparent;
  color: var(--positive);
  font-size: 10px;
  font-weight: 700;
  min-height: 44px;
  padding: 0;
}

.earn-mask {
  align-items: end;
  background: var(--overlay);
  display: grid;
  inset: 0;
  position: fixed;
  z-index: var(--layer-overlay);
}

.earn-dialog {
  background: var(--surface-elevated);
  border-radius: 20px 20px 0 0;
  box-shadow: none;
  display: grid;
  gap: 16px;
  padding: 18px 16px calc(18px + env(safe-area-inset-bottom));
  width: 100%;
}

.earn-dialog > header {
  align-items: start;
  display: flex;
  justify-content: space-between;
}

.earn-dialog > header div {
  display: grid;
  gap: 5px;
}

.earn-dialog > header small,
.earn-availability {
  color: var(--muted);
  font-size: 10px;
}

.earn-availability,
.dialog-feedback {
  margin: 0;
}

.dialog-feedback {
  color: var(--negative);
  font-size: 11px;
}

@media (max-width: 340px) {
  .earn-hero-pencil h1 {
    font-size: 20px;
  }

  .earn-product-pencil dl {
    gap: 6px;
  }

  .earn-product-pencil dd {
    font-size: 10px;
  }
}
</style>
