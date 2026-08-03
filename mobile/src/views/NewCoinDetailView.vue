<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  CheckCircle2,
  CircleAlert,
  LoaderCircle,
  PackageOpen,
  RefreshCw,
  Share2,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
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
import { useModalDialog } from '@/core/modalDialog'
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
const shareFeedback = ref('')
const reviewOpen = ref(false)
const reviewDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapReviewFocus } = useModalDialog(reviewOpen, reviewDialog, '[data-dialog-cancel]')

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

const unlockSummary = computed(() => {
  const current = project.value
  if (!current) return '--'
  const parts = [unlockTypeLabel(current.unlockType)]
  if (current.fixedUnlockAt) parts.push(formatDateTime(current.fixedUnlockAt))
  else if (current.relativeUnlockSeconds) parts.push(t('newCoin.days', { days: Math.ceil(current.relativeUnlockSeconds / 86400) }))
  const fee = current.unlockFeeEnabled
    ? `${formatAmount(current.unlockFeeRate || 0)} ${current.unlockFeeBasis || ''}`.trim()
    : t('newCoin.none')
  parts.push(`${t('newCoin.unlockFee')}: ${fee}`)
  return parts.join(' · ')
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

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: router.currentRoute.value.fullPath } })
}

async function shareProject(): Promise<void> {
  const data = { title: project.value?.symbol || props.symbol.toUpperCase(), url: window.location.href }
  try {
    if (navigator.share) await navigator.share(data)
    else await navigator.clipboard.writeText(data.url)
    shareFeedback.value = t('newCoin.linkCopied')
  } catch {
    shareFeedback.value = ''
  }
}

function requestSubmit(): void {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!project.value || !canSubmit.value) {
    error.value = t('newCoin.invalidAmount')
    return
  }
  error.value = ''
  reviewOpen.value = true
}

function closeReview(): void {
  if (submitting.value) return
  reviewOpen.value = false
  error.value = ''
}

async function submit(): Promise<void> {
  if (!project.value || !reviewOpen.value || !canSubmit.value) {
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
    reviewOpen.value = false
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t(canPurchase.value ? 'newCoin.purchaseFailed' : 'newCoin.subscriptionFailed'))
  } finally {
    submitting.value = false
  }
}

function handleReviewKeydown(event: KeyboardEvent): void {
  trapReviewFocus(event, closeReview)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain pencil-page new-coin-detail-pencil" data-pencil-source="nFwYy B6Qh9J">
    <PageHeader :back="true" :pencil="true" :title="t('newCoin.detailTitle')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('newCoin.shareProject')" @click="shareProject"><Share2 :size="18" /></button>
      </template>
    </PageHeader>

    <div class="pencil-content new-coin-detail-pencil__content">
      <div v-if="success" class="pencil-message pencil-message--success" role="status"><CheckCircle2 :size="18" /><span>{{ success }}</span></div>
      <div v-if="shareFeedback" class="pencil-message pencil-message--success" role="status"><Share2 :size="17" /><span>{{ shareFeedback }}</span></div>
      <div v-if="loading" class="pencil-state" aria-live="polite"><LoaderCircle :size="24" class="spin" /><span>{{ t('newCoin.loadingProject') }}</span></div>
      <div v-else-if="error && !project && !reviewOpen" class="pencil-message pencil-message--error" role="alert">
        <CircleAlert :size="18" /><span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="17" /></button>
      </div>

      <template v-else-if="project">
        <div v-if="error && !reviewOpen" class="pencil-message pencil-message--error" role="alert">
          <CircleAlert :size="18" /><span>{{ error }}</span>
        </div>
        <section class="new-coin-detail-hero">
          <AssetMark :symbol="project.symbol" :size="54" />
          <div>
            <h1>{{ project.symbol }}</h1>
            <span :class="{ 'is-closed': lifecycle === 'closed' }">{{ lifecycleLabel }}</span>
          </div>
        </section>

        <dl class="new-coin-facts">
          <div><dt>{{ t('newCoin.issuePrice') }}</dt><dd class="pencil-numeric">{{ formatPrice(project.issuePrice) }} {{ quoteSymbol }}</dd></div>
          <div><dt>{{ t('newCoin.listingTime') }}</dt><dd>{{ project.listedAt ? formatDateTime(project.listedAt) : t('newCoin.pendingSchedule') }}</dd></div>
          <div><dt>{{ t('newCoin.plannedIssue') }}</dt><dd class="pencil-numeric">{{ formatAmount(project.totalSupply) }} {{ project.symbol }}</dd></div>
          <div><dt>{{ t('newCoin.unlockMethod') }}</dt><dd :title="unlockSummary">{{ unlockSummary }}</dd></div>
        </dl>

        <section class="new-coin-process">
          <h2>{{ t('newCoin.processTitle') }}</h2>
          <ol>
            <li><span>01</span><strong>{{ t('newCoin.subscribe') }}</strong><small class="sr-only">{{ t('newCoin.processChooseDescription') }}</small></li>
            <li><span>02</span><strong>{{ t('newCoin.waitingDistribution') }}</strong><small class="sr-only">{{ t('newCoin.processConfirmDescription') }}</small></li>
            <li><span>03</span><strong>{{ t('newCoin.listed') }}</strong><small class="sr-only">{{ t('newCoin.processDistributionDescription') }}</small></li>
          </ol>
        </section>

        <section class="new-coin-entry-pencil">
          <header>
            <span>{{ t(canPurchase ? 'newCoin.purchaseQuantity' : 'newCoin.subscriptionAmount', { asset: project.symbol }) }}</span>
            <button v-if="session.isAuthenticated && (canSubscribe || canPurchase)" type="button" @click="setAmount(1)">{{ t('newCoin.maximum') }}</button>
          </header>
          <div class="new-coin-entry-pencil__control">
            <input v-model="amount" class="pencil-numeric" inputmode="decimal" placeholder="0.00" :disabled="!session.isAuthenticated || (!canSubscribe && !canPurchase)" aria-describedby="new-coin-entry-summary" />
            <span class="new-coin-entry-pencil__asset">
              <select v-if="canSubscribe" v-model="quoteAssetId">
                <option v-if="!accounts.length" :value="0">{{ quoteSymbol }}</option>
                <option v-for="account in accounts" :key="account.assetId" :value="account.assetId">{{ t('newCoin.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option>
              </select>
              <template v-else>{{ canPurchase ? project.symbol : selectedAccount?.symbol || quoteSymbol }}</template>
            </span>
          </div>
          <dl id="new-coin-entry-summary" class="sr-only">
            <div><dt>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</dt><dd class="pencil-numeric">{{ formatAmount(canSubscribe ? estimatedQuantity : paymentAmount) }} {{ canSubscribe ? project.symbol : selectedAccount?.symbol || quoteSymbol }}</dd></div>
            <div v-if="selectedAccount"><dt>{{ t('newCoin.availableBalance') }}</dt><dd class="pencil-numeric">{{ formatAmount(selectedAccount.available) }} {{ selectedAccount.symbol }}</dd></div>
          </dl>
        </section>

        <button
          class="pencil-primary pencil-primary--full new-coin-detail-primary"
          type="button"
          :disabled="session.isAuthenticated && (submitting || (!canSubscribe && !canPurchase) || !canSubmit)"
          :aria-busy="submitting"
          @click="requestSubmit"
        >
          {{ submitting
            ? t('common.submitting')
            : !session.isAuthenticated
              ? t('auth.login')
              : t(canSubscribe ? 'newCoin.subscribeAsset' : canPurchase ? 'newCoin.purchaseAsset' : 'newCoin.stageUnavailable', { asset: project.symbol }) }}
        </button>
      </template>
      <div v-else class="pencil-state"><PackageOpen :size="23" /><span>{{ t('newCoin.noProjects') }}</span></div>
    </div>

    <div v-if="reviewOpen && project && selectedAccount" class="entry-review-mask" @click.self="closeReview">
      <section
        ref="reviewDialog"
        class="entry-review"
        role="dialog"
        aria-modal="true"
        aria-labelledby="entry-review-title"
        aria-describedby="entry-review-description"
        @keydown="handleReviewKeydown"
      >
        <header>
          <div>
            <span>{{ t(canSubscribe ? 'newCoin.subscribe' : 'newCoin.postListingPurchase') }}</span>
            <h2 id="entry-review-title">{{ t(canSubscribe ? 'newCoin.subscribeTitle' : 'newCoin.purchaseTitle') }}</h2>
            <small id="entry-review-description">{{ canSubscribe ? t('newCoin.subscribeDescription') : t('newCoin.purchaseDescription', { price: formatPrice(executionPrice), asset: quoteSymbol }) }}</small>
          </div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="submitting" @click="closeReview"><X :size="21" /></button>
        </header>
        <dl class="entry-review__summary">
          <div><dt>{{ t('newCoin.paymentAsset') }}</dt><dd>{{ selectedAccount.symbol }}</dd></div>
          <div><dt>{{ t(canSubscribe ? 'newCoin.subscriptionAmount' : 'newCoin.purchaseQuantity', { asset: project.symbol }) }}</dt><dd class="pencil-numeric">{{ formatAmount(amountNumber) }} {{ canSubscribe ? selectedAccount.symbol : project.symbol }}</dd></div>
          <div><dt>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</dt><dd class="pencil-numeric up">{{ formatAmount(canSubscribe ? estimatedQuantity : paymentAmount) }} {{ canSubscribe ? project.symbol : selectedAccount.symbol }}</dd></div>
          <div><dt>{{ t('newCoin.availableBalance') }}</dt><dd class="pencil-numeric">{{ formatAmount(selectedAccount.available) }} {{ selectedAccount.symbol }}</dd></div>
        </dl>
        <p v-if="error" class="entry-review__error" role="alert">{{ error }}</p>
        <div class="entry-review__actions">
          <button class="pencil-secondary" type="button" :disabled="submitting" data-dialog-cancel @click="closeReview">{{ t('common.cancel') }}</button>
          <button class="pencil-primary" type="button" :disabled="submitting || !canSubmit" :aria-busy="submitting" @click="submit">{{ submitting ? t('common.submitting') : t(canSubscribe ? 'newCoin.subscribeAsset' : 'newCoin.purchaseAsset', { asset: project.symbol }) }}</button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.new-coin-detail-pencil__content {
  display: flow-root;
  min-height: 516px;
  padding-top: 0;
}

.new-coin-detail-hero {
  align-items: center;
  display: grid;
  gap: 14px;
  grid-template-columns: 54px minmax(0, 1fr);
  height: 56px;
  margin-top: 8px;
}

.new-coin-detail-hero > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.new-coin-detail-hero h1 {
  font-size: 23px;
  font-weight: 500;
  letter-spacing: -.02em;
  line-height: 29px;
  margin: 0;
}

.new-coin-detail-hero > div > span {
  color: var(--positive);
  font-size: 10px;
  font-weight: 600;
  line-height: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-hero > div > span.is-closed {
  color: var(--negative);
}

.new-coin-facts {
  display: grid;
  grid-template-rows: repeat(4, 39px);
  height: 156px;
  margin: 18px 0 0;
}

.new-coin-facts > div {
  align-items: center;
  display: flex;
  gap: 16px;
  justify-content: space-between;
  min-width: 0;
}

.new-coin-facts dt {
  color: var(--muted);
  flex: 0 0 auto;
  font-size: 11px;
  line-height: 16px;
}

.new-coin-facts dd {
  font-size: 10px;
  font-weight: 650;
  line-height: 16px;
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-process {
  margin-top: 18px;
}

.new-coin-process h2 {
  font-size: 14px;
  font-weight: 700;
  height: 22px;
  line-height: 22px;
  margin: 0;
}

.new-coin-process ol {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 65px;
  list-style: none;
  margin: 18px 0 0;
  padding: 0;
}

.new-coin-process li {
  align-items: center;
  display: flex;
  flex-direction: column;
  min-width: 0;
  text-align: center;
}

.new-coin-process li > span {
  color: var(--positive);
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 11px;
  font-weight: 700;
  line-height: 15px;
}

.new-coin-process li strong {
  font-size: 11px;
  font-weight: 500;
  line-height: 16px;
  margin-top: 10px;
  max-width: 100%;
}

.new-coin-entry-pencil {
  height: 51px;
  margin-top: 18px;
}

.new-coin-entry-pencil > header {
  align-items: center;
  display: flex;
  height: 15px;
  justify-content: space-between;
  position: relative;
}

.new-coin-entry-pencil > header span {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.new-coin-entry-pencil > header button {
  background: transparent;
  color: var(--positive);
  font-size: 10px;
  font-weight: 600;
  min-height: 44px;
  padding: 0;
  position: absolute;
  right: 0;
  top: -14px;
}

.new-coin-entry-pencil__control {
  align-items: center;
  border-bottom: 1px solid transparent;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 32px;
  margin-top: 4px;
}

.new-coin-entry-pencil__control:focus-within {
  border-bottom-color: var(--positive);
}

.new-coin-entry-pencil__control input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 22px;
  font-weight: 700;
  height: 32px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.new-coin-entry-pencil__control input:disabled {
  opacity: 1;
}

.new-coin-entry-pencil__asset {
  color: var(--muted);
  font-size: 10px;
  max-width: 142px;
  min-width: 0;
}

.new-coin-entry-pencil__asset select {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--muted);
  font-size: 10px;
  max-width: 142px;
  outline: 0;
  text-align: right;
  text-overflow: ellipsis;
}

.new-coin-detail-primary {
  height: 48px;
  margin-top: 18px;
  min-height: 48px;
}

.new-coin-detail-primary:disabled {
  background: var(--accent-soft);
  color: var(--positive);
  opacity: 1;
}

.new-coin-detail-pencil :deep(.asset-mark) {
  --asset-color: var(--accent);
  --asset-ink: var(--on-accent);
  background: var(--accent);
  border: 0;
  box-shadow: none;
  color: var(--on-accent);
}

.entry-review-mask {
  align-items: end;
  background: var(--overlay);
  display: grid;
  inset: 0;
  position: fixed;
  z-index: var(--layer-overlay);
}

.entry-review {
  background: var(--surface-elevated);
  border-radius: 20px 20px 0 0;
  box-shadow: none;
  padding: 18px 16px calc(18px + env(safe-area-inset-bottom));
  width: 100%;
}

.entry-review > header {
  align-items: start;
  display: flex;
  justify-content: space-between;
}

.entry-review > header div {
  display: grid;
  gap: 4px;
}

.entry-review > header span,
.entry-review > header small {
  color: var(--muted);
  font-size: 9px;
}

.entry-review h2 {
  font-size: 18px;
  margin: 0;
}

.entry-review__summary {
  display: grid;
  gap: 10px;
  margin: 17px 0;
}

.entry-review__summary > div {
  align-items: center;
  display: flex;
  font-size: 10px;
  justify-content: space-between;
}

.entry-review__summary dt {
  color: var(--muted);
}

.entry-review__summary dd {
  margin: 0;
  text-align: right;
}

.entry-review__error {
  color: var(--negative);
  font-size: 11px;
}

.entry-review__actions {
  display: grid;
  gap: 10px;
  grid-template-columns: 1fr 1fr;
}

@media (max-width: 340px) {
  .new-coin-detail-hero {
    gap: 10px;
    grid-template-columns: 50px minmax(0, 1fr);
  }

  .new-coin-detail-hero h1 {
    font-size: 21px;
  }

  .new-coin-facts > div {
    gap: 10px;
  }

  .new-coin-process ol {
    gap: 6px;
  }

  .new-coin-process li strong {
    font-size: 10px;
  }
}
</style>
