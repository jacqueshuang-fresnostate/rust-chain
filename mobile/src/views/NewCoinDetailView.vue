<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowUpRight,
  CalendarDays,
  CheckCircle2,
  CircleAlert,
  CircleDollarSign,
  Clock3,
  LoaderCircle,
  PackageOpen,
  ReceiptText,
  RefreshCw,
  ShieldCheck,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  createNewCoinPurchase,
  fetchNewCoinProject,
  subscribeNewCoin,
  type NewCoinProject,
} from '@/api/newCoin'
import { fetchMarketTickers } from '@/api/market'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { newCoinPurchaseQuantity } from '@/core/newCoinPurchase'
import type { MarketTicker, WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

const props = defineProps<{ symbol: string }>()
const router = useRouter()
const { t } = useI18n()
const session = useSessionStore()
const project = ref<NewCoinProject | null>(null)
const accounts = ref<WalletAccount[]>([])
const tickers = ref<MarketTicker[]>([])
const quoteAssetId = ref(0)
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')

const lifecycle = computed(() => project.value?.lifecycleStatus.toLowerCase() || '')
const canSubscribe = computed(() => lifecycle.value === 'subscription')
const canPurchase = computed(() => lifecycle.value === 'listed' && Boolean(project.value?.postListingPurchaseEnabled && project.value?.postListingPairId))
const selectedTicker = computed(() => tickers.value.find((ticker) => ticker.id === project.value?.postListingPairId))
const quoteSymbol = computed(() => canPurchase.value ? selectedTicker.value?.quote || t('newCoin.quoteAsset') : 'USDT')
const selectedAccount = computed(() => canPurchase.value
  ? accounts.value.find((account) => account.symbol === selectedTicker.value?.quote)
  : accounts.value.find((account) => account.assetId === quoteAssetId.value))
const amountNumber = computed(() => Number(amount.value || 0))
const executionPrice = computed(() => selectedTicker.value?.lastPrice || project.value?.issuePrice || 0)
const paymentAmount = computed(() => canPurchase.value ? amountNumber.value * executionPrice.value : amountNumber.value)
const estimatedQuantity = computed(() => canSubscribe.value && project.value?.issuePrice ? amountNumber.value / project.value.issuePrice : amountNumber.value)
const canSubmit = computed(() => {
  if (!project.value || !selectedAccount.value || !Number.isFinite(amountNumber.value) || amountNumber.value <= 0) return false
  if (paymentAmount.value > selectedAccount.value.available) return false
  if (canSubscribe.value) return estimatedQuantity.value > 0
  return canPurchase.value && executionPrice.value > 0
})

const lifecycleLabel = computed(() => {
  const keys: Record<string, string> = {
    subscription: 'newCoin.subscriptionOpen',
    distribution: 'newCoin.waitingDistribution',
    listed: 'newCoin.listed',
    closed: 'newCoin.closed',
  }
  const key = keys[lifecycle.value]
  return key ? t(key) : project.value?.lifecycleStatus || '--'
})

function unlockTypeLabel(type: string): string {
  const keys: Record<string, string> = {
    fixed: 'newCoin.fixedUnlock',
    relative: 'newCoin.relativeUnlock',
  }
  const key = keys[type.toLowerCase()]
  return key ? t(key) : type || t('newCoin.unlockPending')
}

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const nextProject = await fetchNewCoinProject(props.symbol)
    project.value = nextProject
    const requests: [Promise<WalletAccount[]>, Promise<MarketTicker[]>] = [
      session.isAuthenticated ? fetchWalletAccounts() : Promise.resolve([]),
      nextProject.postListingPurchaseEnabled ? fetchMarketTickers() : Promise.resolve([]),
    ]
    const [nextAccounts, nextTickers] = await Promise.all(requests)
    accounts.value = nextAccounts
    tickers.value = nextTickers
    selectDefaultAccount()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.projectLoadFailed'))
  } finally {
    loading.value = false
  }
}

function selectDefaultAccount(): void {
  const matching = accounts.value.find((account) => account.symbol === quoteSymbol.value)
  quoteAssetId.value = matching?.assetId || accounts.value.find((account) => account.symbol === 'USDT')?.assetId || accounts.value[0]?.assetId || 0
}

function setAmount(value: number): void {
  const available = selectedAccount.value?.available || 0
  const next = canPurchase.value
    ? newCoinPurchaseQuantity(available, value, executionPrice.value)
    : Math.max(0, Math.min(available * value, available))
  amount.value = next ? String(Number(next.toFixed(8))) : ''
}

async function submit(): Promise<void> {
  if (!project.value || !canSubmit.value) {
    error.value = t('newCoin.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    if (canSubscribe.value) {
      await subscribeNewCoin({
        symbol: project.value.symbol,
        quoteAssetId: quoteAssetId.value,
        quoteAmount: amountNumber.value,
        issuePrice: project.value.issuePrice,
      })
      success.value = t('newCoin.subscriptionSubmitted')
    } else if (canPurchase.value && project.value.postListingPairId) {
      await createNewCoinPurchase({
        symbol: project.value.symbol,
        pairId: project.value.postListingPairId,
        price: executionPrice.value,
        quantity: amountNumber.value,
      })
      success.value = t('newCoin.purchaseSubmitted')
    }
    amount.value = ''
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t(canPurchase.value ? 'newCoin.purchaseFailed' : 'newCoin.subscriptionFailed'))
  } finally {
    submitting.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain new-coin-detail-page">
    <PageHeader :title="t('newCoin.projectTitle', { symbol: props.symbol.toUpperCase() })">
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('newCoin.records')"
          @click="router.push({ name: 'new-coin-records' })"
        >
          <ReceiptText :size="20" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content new-coin-detail-content">
      <div v-if="error" class="detail-message detail-message--error" role="alert">
        <CircleAlert :size="18" />
        <span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load">
          <RefreshCw :size="17" />
        </button>
      </div>
      <div v-if="success" class="detail-message detail-message--success" role="status">
        <CheckCircle2 :size="18" />
        <span>{{ success }}</span>
      </div>
      <div v-if="loading" class="detail-state" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <span>{{ t('newCoin.loadingProject') }}</span>
      </div>
      <template v-else-if="project">
        <section class="project-hero">
          <AssetMark :symbol="project.symbol" :size="56" />
          <div>
            <span>{{ lifecycleLabel }}</span>
            <h1>{{ project.symbol }}</h1>
            <p>{{ t('newCoin.projectDescription') }}</p>
          </div>
          <ShieldCheck :size="20" />
        </section>

        <section class="project-metrics">
          <article><span>{{ t('newCoin.issuePrice') }}</span><strong class="numeric">{{ formatPrice(project.issuePrice) }}</strong><small>{{ t('newCoin.referenceAsset', { asset: quoteSymbol }) }}</small></article>
          <article><span>{{ t('newCoin.plannedIssue') }}</span><strong class="numeric">{{ formatAmount(project.totalSupply) }}</strong><small>{{ project.symbol }}</small></article>
          <article><span>{{ t('newCoin.currentStage') }}</span><strong>{{ lifecycleLabel }}</strong><small>{{ project.status || lifecycleLabel }}</small></article>
        </section>

        <section class="project-section">
          <div class="section-heading"><span>{{ t('newCoin.rules') }}</span></div>
          <dl class="detail-list">
            <div><dt><CalendarDays :size="17" />{{ t('newCoin.listingTime') }}</dt><dd>{{ formatDateTime(project.listedAt) }}</dd></div>
            <div><dt><Clock3 :size="17" />{{ t('newCoin.unlockMethod') }}</dt><dd>{{ unlockTypeLabel(project.unlockType) }}</dd></div>
            <div v-if="project.fixedUnlockAt"><dt><ArrowUpRight :size="17" />{{ t('newCoin.estimatedUnlock') }}</dt><dd>{{ formatDateTime(project.fixedUnlockAt) }}</dd></div>
            <div v-else-if="project.relativeUnlockSeconds"><dt><ArrowUpRight :size="17" />{{ t('newCoin.unlockPeriod') }}</dt><dd>{{ t('newCoin.days', { days: Math.ceil(project.relativeUnlockSeconds / 86400) }) }}</dd></div>
            <div><dt><ShieldCheck :size="17" />{{ t('newCoin.unlockFee') }}</dt><dd>{{ project.unlockFeeEnabled ? `${formatAmount(project.unlockFeeRate)} ${project.unlockFeeBasis || ''}` : t('newCoin.none') }}</dd></div>
          </dl>
        </section>

        <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.detailLoginDescription')" />
        <section v-else-if="canSubscribe || canPurchase" class="entry-panel">
          <header>
            <div>
              <span>{{ t(canSubscribe ? 'newCoin.subscribe' : 'newCoin.postListingPurchase') }}</span>
              <h2>{{ t(canSubscribe ? 'newCoin.subscribeTitle' : 'newCoin.purchaseTitle') }}</h2>
            </div>
            <CircleDollarSign :size="23" />
          </header>
          <p>{{ canSubscribe ? t('newCoin.subscribeDescription') : t('newCoin.purchaseDescription', { price: formatPrice(executionPrice), asset: quoteSymbol }) }}</p>
          <label class="entry-field">
            <span>{{ t('newCoin.paymentAsset') }}</span>
            <select v-if="canSubscribe" v-model="quoteAssetId">
              <option v-for="account in accounts" :key="account.assetId" :value="account.assetId">
                {{ t('newCoin.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}
              </option>
            </select>
            <div v-else class="entry-field__locked">
              <b>{{ quoteSymbol }}</b>
              <small>{{ t('newCoin.purchaseQuoteLocked') }}</small>
            </div>
          </label>
          <label class="entry-field">
            <span>{{ t(canSubscribe ? 'newCoin.subscriptionAmount' : 'newCoin.purchaseQuantity', { asset: project.symbol }) }}</span>
            <div>
              <input v-model="amount" class="numeric" inputmode="decimal" placeholder="0.00" />
              <b>{{ canSubscribe ? selectedAccount?.symbol || quoteSymbol : project.symbol }}</b>
            </div>
          </label>
          <div class="quick-values">
            <button v-for="value in [0.25, 0.5, 0.75, 1]" :key="value" type="button" @click="setAmount(value)">
              {{ value === 1 ? t('newCoin.maximum') : `${value * 100}%` }}
            </button>
          </div>
          <dl class="entry-summary">
            <div><dt>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</dt><dd>{{ formatAmount(canSubscribe ? estimatedQuantity : paymentAmount) }} {{ canSubscribe ? project.symbol : selectedAccount?.symbol || quoteSymbol }}</dd></div>
            <div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatAmount(selectedAccount?.available) }} {{ selectedAccount?.symbol }}</dd></div>
          </dl>
          <button
            class="button button--primary button--full entry-submit"
            type="button"
            :disabled="submitting || !canSubmit"
            :aria-busy="submitting"
            @click="submit"
          >
            {{ submitting ? t('common.submitting') : t(canSubscribe ? 'newCoin.subscribeAsset' : 'newCoin.purchaseAsset', { asset: project.symbol }) }}
          </button>
        </section>
        <section v-else class="stage-note">
          <Clock3 :size="19" />
          <div>
            <strong>{{ t('newCoin.stageUnavailable') }}</strong>
            <p>{{ t('newCoin.stageUnavailableDescription') }}</p>
          </div>
        </section>
      </template>
      <div v-else class="detail-state detail-state--empty">
        <PackageOpen :size="23" />
        <span>{{ t('newCoin.noProjects') }}</span>
      </div>
    </div>
  </main>
</template>

<style scoped>
.new-coin-detail-page {
  background: var(--surface);
  min-width: 0;
}

.new-coin-detail-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.detail-message {
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

.detail-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.detail-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.detail-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.detail-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 148px;
  text-align: center;
}

.detail-state--empty {
  min-height: 112px;
}

.project-hero {
  align-items: center;
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--accent) 9%, transparent), transparent 64%),
    var(--surface);
  border-block: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 13px;
  grid-template-columns: 56px minmax(0, 1fr) auto;
  min-height: 106px;
  padding: 14px 4px;
}

.project-hero > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.project-hero span {
  color: var(--accent);
  font-size: 10px;
  font-weight: 800;
}

.project-hero h1 {
  font-size: 24px;
  line-height: 1;
  margin: 0;
}

.project-hero p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.project-hero > svg {
  color: var(--positive);
}

.project-metrics {
  border-block: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.project-metrics article {
  display: grid;
  gap: 5px;
  min-height: 96px;
  min-width: 0;
  padding: 14px 10px;
}

.project-metrics article + article {
  border-left: 1px solid var(--line);
}

.project-metrics span,
.project-metrics small {
  color: var(--muted);
  font-size: 9px;
}

.project-metrics strong {
  font-size: 13px;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.project-section {
  border-top: 8px solid var(--soft);
  margin: 6px -20px 0;
  padding: 0 20px;
}

.project-section .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 54px;
}

.detail-list {
  display: grid;
  margin: 0;
}

.detail-list > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 18px;
  justify-content: space-between;
  min-height: 52px;
}

.detail-list dt {
  align-items: center;
  color: var(--muted-strong);
  display: inline-flex;
  font-size: 11px;
  gap: 8px;
}

.detail-list dt svg {
  color: var(--accent);
  flex: 0 0 auto;
}

.detail-list dd {
  color: var(--ink);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  margin: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.entry-panel {
  border-block: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 13px;
  padding: 15px 0 0;
}

.entry-panel > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}

.entry-panel > header > div {
  display: grid;
  gap: 4px;
}

.entry-panel > header span {
  color: var(--accent);
  font-size: 10px;
  font-weight: 800;
}

.entry-panel h2 {
  font-size: 18px;
  margin: 0;
}

.entry-panel > header > svg {
  color: var(--accent);
}

.entry-panel > p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: -2px 0 2px;
}

.entry-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.entry-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.entry-field > span {
  color: var(--muted);
  font-size: 10px;
}

.entry-field > div {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 44px;
}

.entry-field__locked small {
  color: var(--muted);
  font-size: 11px;
  text-align: right;
}

.entry-field select,
.entry-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-height: 44px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.entry-field select {
  font-size: 12px;
}

.entry-field input {
  font-size: 20px;
  font-weight: 750;
}

.entry-field b {
  font-size: 12px;
}

.quick-values {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.quick-values button {
  background: var(--surface);
  border: 1px solid var(--line);
  color: var(--ink);
  font-size: 11px;
  min-height: 44px;
}

.quick-values button:focus-visible,
.quick-values button:hover {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent);
}

.entry-summary {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.entry-summary > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.entry-summary > div:last-child {
  border-bottom: 0;
}

.entry-summary dt,
.entry-summary dd {
  font-size: 11px;
  margin: 0;
}

.entry-summary dt {
  color: var(--muted);
}

.entry-summary dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  overflow-wrap: anywhere;
  text-align: right;
}

.entry-submit {
  border-radius: 0;
  min-height: 52px;
}

.stage-note {
  align-items: flex-start;
  background: var(--soft);
  border-left: 3px solid var(--muted-strong);
  display: flex;
  gap: 10px;
  padding: 15px;
}

.stage-note > svg {
  color: var(--muted-strong);
  flex: 0 0 auto;
  margin-top: 2px;
}

.stage-note > div {
  display: grid;
  gap: 5px;
}

.stage-note strong {
  font-size: 13px;
}

.stage-note p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .new-coin-detail-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .project-section {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .project-hero {
    grid-template-columns: 48px minmax(0, 1fr);
  }

  .project-hero > svg {
    display: none;
  }

  .project-metrics {
    grid-template-columns: 1fr;
  }

  .project-metrics article {
    grid-template-columns: minmax(0, 1fr) auto;
    min-height: 56px;
  }

  .project-metrics article + article {
    border-left: 0;
    border-top: 1px solid var(--line);
  }

  .project-metrics small {
    grid-column: 1 / -1;
  }

  .detail-list > div,
  .entry-summary > div {
    align-items: flex-start;
    flex-direction: column;
    gap: 5px;
    justify-content: center;
    padding: 8px 0;
  }

  .detail-list dd,
  .entry-summary dd {
    text-align: left;
  }

  .quick-values {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
