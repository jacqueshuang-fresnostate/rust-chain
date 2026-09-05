<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  ArrowRight,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clock3,
  Flame,
  LineChart,
  LoaderCircle,
  PackageCheck,
  PackageOpen,
  RefreshCw,
  Share2,
  Tag,
  TicketCheck,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  createNewCoinPurchase,
  fetchNewCoinProject,
  subscribeNewCoin,
  type NewCoinProject,
} from '@/api/newCoin'
import { fetchWalletAccounts } from '@/api/wallet'
import {
  decimalCompare,
  decimalDivide,
  decimalMultiply,
  decimalPortion,
  decimalTextFromBoundary,
  formatDecimalText,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalText,
} from '@/core/decimal'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { formatDateTime } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
import {
  newCoinLifecycleMilestone,
  newCoinProjectProgress,
  newCoinUnlockTypeTranslationKey,
} from '@/core/newCoinPresentation'
import type { WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

const props = defineProps<{ symbol: string }>()
const router = useRouter()
const { locale, t } = useI18n()
const session = useSessionStore()
const project = ref<NewCoinProject | null>(null)
const accounts = ref<WalletAccount[]>([])
const amount = ref('')
const selectedPercentage = ref<25 | 50 | 75 | 100 | null>(null)
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
const canPurchase = computed(() => lifecycle.value === 'listed'
  && Boolean(project.value?.postListingPurchaseEnabled && project.value?.postListingPairId))
const quoteSymbol = computed(() => project.value?.quoteAssetSymbol || t('newCoin.unavailableValue'))
const projectName = computed(() => project.value?.name || t('newCoin.projectNameUnavailable'))
const selectedAccount = computed(() => accounts.value.find(
  (account) => account.assetId === project.value?.quoteAssetId,
))
const amountText = computed(() => positiveDecimalInput(amount.value))
const availableText = computed(() => decimalTextFromBoundary(
  selectedAccount.value?.availableText,
  { allowNegative: false },
))
// Both subscription and post-listing purchase are settled by the backend at
// the project's authoritative issue price; a public ticker is display data,
// not a client-controlled execution quote.
const executionPriceText = computed<DecimalText | null>(() => project.value?.issuePriceText || null)
const paymentAmount = computed<DecimalText | null>(() => amountText.value
  && (canPurchase.value && executionPriceText.value
    ? decimalMultiply(amountText.value, executionPriceText.value)
    : amountText.value))
const estimatedQuantity = computed<DecimalText | null>(() => amountText.value
  && (canSubscribe.value && project.value
    ? decimalDivide(amountText.value, project.value.issuePriceText, 18)
    : amountText.value))
const canSubmit = computed(() => {
  if (!project.value || !project.value.quoteAssetId || !selectedAccount.value) return false
  if (!availableText.value || !amountText.value || !paymentAmount.value || !executionPriceText.value) return false
  if (decimalCompare(paymentAmount.value, availableText.value) > 0) return false
  if (canSubscribe.value) return Boolean(estimatedQuantity.value && estimatedQuantity.value !== '0')
  return canPurchase.value
})
const lifecycleLabel = computed(() => statusLabel(project.value?.lifecycleStatus || ''))
const milestone = computed(() => newCoinLifecycleMilestone(project.value?.lifecycleStatus || ''))
const progress = computed(() => project.value ? newCoinProjectProgress(project.value) : null)
const progressText = computed(() => progress.value
  ? formatDecimalText(
    decimalMultiply(progress.value.ratio, normalizeDecimalText('100')),
    locale.value,
    { maximumFractionDigits: 2 },
  )
  : t('newCoin.unavailableValue'))
const stageItems = computed(() => [
  { key: 'preheat', label: t('newCoin.preheat'), icon: Flame },
  { key: 'subscription', label: t('newCoin.subscribe'), icon: TicketCheck },
  { key: 'distribution', label: t('newCoin.pendingListing'), icon: PackageCheck },
  { key: 'listed', label: t('newCoin.listed'), icon: LineChart },
])
const unlockSummary = computed(() => {
  const current = project.value
  if (!current) return t('newCoin.unavailableValue')
  if (current.fixedUnlockAt) return formatDateTime(current.fixedUnlockAt)
  if (current.relativeUnlockSeconds !== undefined) {
    return t('newCoin.days', { days: Math.ceil(current.relativeUnlockSeconds / 86400) })
  }
  return unlockTypeLabel(current.unlockType)
})
const unlockFeeSummary = computed(() => {
  const current = project.value
  if (!current?.unlockFeeEnabled) return t('newCoin.none')
  const rate = current.unlockFeeRateText
    ? formatFinancialAmount(current.unlockFeeRateText, locale.value, { maximumFractionDigits: 8 })
    : t('newCoin.unavailableValue')
  return current.unlockFeeBasis ? `${rate} · ${current.unlockFeeBasis}` : rate
})
const actionLabel = computed(() => {
  if (!canSubscribe.value && !canPurchase.value) return t('newCoin.stageUnavailable')
  if (!session.isAuthenticated) return t('auth.login')
  if (!selectedAccount.value && (canSubscribe.value || canPurchase.value)) return t('newCoin.quoteAccountUnavailable')
  if (canSubscribe.value) return t('newCoin.subscribeAsset', { asset: project.value?.symbol || '' })
  if (canPurchase.value) return t('newCoin.purchaseAsset', { asset: project.value?.symbol || '' })
  return t('newCoin.stageUnavailable')
})

function formatMoney(value: DecimalText | null | undefined, assetSymbol?: string): string {
  return value
    ? formatFinancialAmount(value, locale.value, { assetSymbol })
    : t('newCoin.unavailableValue')
}

function statusLabel(status: string): string {
  const key = ({
    preheat: 'newCoin.preheat',
    subscription: 'newCoin.subscriptionOpen',
    distribution: 'newCoin.waitingDistribution',
    listed: 'newCoin.listed',
    closed: 'newCoin.closed',
  } as Record<string, string>)[status.toLowerCase()]
  return key ? t(key) : status || t('newCoin.unavailableValue')
}

function unlockTypeLabel(type: string): string {
  const key = newCoinUnlockTypeTranslationKey(type)
  return key ? t(key) : type || t('newCoin.unlockPending')
}

async function load(): Promise<void> {
  loading.value = !project.value
  error.value = ''
  try {
    const nextProject = await fetchNewCoinProject(props.symbol)
    project.value = nextProject
    accounts.value = session.isAuthenticated ? await fetchWalletAccounts() : []
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.projectLoadFailed'))
  } finally {
    loading.value = false
  }
}

function setAmount(percentage: 25 | 50 | 75 | 100): void {
  selectedPercentage.value = percentage
  if (!availableText.value) {
    amount.value = ''
    return
  }
  const budget = decimalPortion(availableText.value, percentage, 100, 18)
  const next = canPurchase.value && executionPriceText.value
    ? decimalDivide(budget, executionPriceText.value, 18)
    : budget
  amount.value = next === '0' ? '' : next
}

function onAmountInput(): void {
  selectedPercentage.value = null
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
    const requestAmount = amountText.value
    if (!requestAmount) throw new TypeError('invalid new-coin amount')
    if (canSubscribe.value && project.value.quoteAssetId) {
      await subscribeNewCoin({
        symbol: project.value.symbol,
        quoteAssetId: project.value.quoteAssetId,
        quoteAmount: requestAmount,
        issuePrice: project.value.issuePriceText,
      })
      success.value = t('newCoin.subscriptionSubmitted')
    } else if (canPurchase.value && project.value.postListingPairId) {
      await createNewCoinPurchase({
        symbol: project.value.symbol,
        pairId: project.value.postListingPairId,
        price: project.value.issuePriceText,
        quantity: requestAmount,
      })
      success.value = t('newCoin.purchaseSubmitted')
    }
    amount.value = ''
    selectedPercentage.value = null
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
    <PageHeader :back="true" :pencil="true" back-icon="chevron" :title="t('newCoin.detailTitle')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('newCoin.shareProject')" @click="shareProject">
          <Share2 :size="22" />
        </button>
      </template>
    </PageHeader>

    <div v-if="success" class="new-coin-detail-feedback" role="status"><CheckCircle2 :size="17" /><span>{{ success }}</span></div>
    <div v-if="shareFeedback" class="new-coin-detail-feedback" role="status"><Share2 :size="16" /><span>{{ shareFeedback }}</span></div>

    <div v-if="loading" class="new-coin-detail-state" aria-live="polite">
      <LoaderCircle :size="24" class="spin" /><span>{{ t('newCoin.loadingProject') }}</span>
    </div>
    <div v-else-if="error && !project && !reviewOpen" class="new-coin-detail-state" role="alert">
      <CircleAlert :size="24" /><span>{{ error }}</span>
      <button type="button" @click="load"><RefreshCw :size="17" />{{ t('common.retry') }}</button>
    </div>

    <template v-else-if="project">
      <div v-if="error && !reviewOpen" class="new-coin-detail-feedback new-coin-detail-feedback--error" role="alert">
        <CircleAlert :size="17" /><span>{{ error }}</span>
      </div>

      <section class="new-coin-detail-visual">
        <header>
          <AssetMark :symbol="project.symbol" :src="project.logoUrl" :size="52" />
          <span class="new-coin-detail-identity">
            <h1>{{ project.symbol }}</h1>
            <small>{{ projectName }}</small>
          </span>
          <b>{{ lifecycleLabel }}</b>
        </header>
        <p>{{ t('newCoin.projectDataDescription') }}</p>
        <dl class="new-coin-detail-plates">
          <div>
            <dt><Tag :size="17" /><span>{{ t('newCoin.issuePrice') }}</span></dt>
            <dd>{{ formatMoney(project.issuePriceText, project.quoteAssetSymbol) }} {{ quoteSymbol }}</dd>
          </div>
          <div>
            <dt><PackageCheck :size="17" /><span>{{ t('newCoin.plannedIssue') }}</span></dt>
            <dd>{{ formatMoney(project.totalSupplyText, project.symbol) }} {{ project.symbol }}</dd>
          </div>
        </dl>
        <div class="new-coin-detail-progress">
          <span><i :style="{ width: `${progress?.percentage || 0}%` }" /></span>
          <small>{{ t('newCoin.progressRatio', { ratio: progressText }) }}</small>
        </div>
      </section>

      <section class="new-coin-detail-stages">
        <header>
          <h2>{{ t('newCoin.currentStage') }}</h2>
          <small>{{ lifecycleLabel }} · {{ t('newCoin.progressRatio', { ratio: progressText }) }}</small>
        </header>
        <ol>
          <li
            v-for="(stage, index) in stageItems"
            :key="stage.key"
            :class="{ completed: index < milestone, active: index === milestone }"
          >
            <span><component :is="stage.icon" :size="14" /></span>
            <strong>{{ stage.label }}</strong>
          </li>
        </ol>
      </section>

      <section class="new-coin-detail-rules">
        <header><h2>{{ t('newCoin.rules') }}</h2><ChevronRight :size="19" aria-hidden="true" /></header>
        <dl>
          <div><dt>{{ t('newCoin.paymentAsset') }}</dt><dd>{{ quoteSymbol }}</dd></div>
          <div><dt>{{ t('newCoin.listingTime') }}</dt><dd>{{ project.listedAt ? formatDateTime(project.listedAt) : t('newCoin.pendingSchedule') }}</dd></div>
          <div><dt>{{ t('newCoin.unlockMethod') }}</dt><dd :title="`${unlockSummary} · ${unlockFeeSummary}`">{{ unlockSummary }}</dd></div>
        </dl>
      </section>

      <section class="new-coin-detail-entry">
        <h2>{{ t(canPurchase ? 'newCoin.purchaseTitle' : 'newCoin.subscribeTitle') }}</h2>
        <div class="new-coin-detail-balance">
          <span>{{ t('newCoin.availableBalance') }}</span>
          <strong>{{ formatMoney(availableText, quoteSymbol) }} {{ quoteSymbol }}</strong>
        </div>
        <label class="new-coin-detail-amount">
          <span>{{ t(canPurchase ? 'newCoin.purchaseQuantity' : 'newCoin.subscriptionAmount', { asset: project.symbol }) }}</span>
          <input
            v-model="amount"
            inputmode="decimal"
            :placeholder="t('newCoin.amountPlaceholder')"
            :disabled="!session.isAuthenticated || (!canSubscribe && !canPurchase) || !selectedAccount"
            @input="onAmountInput"
          />
          <b>{{ canPurchase ? project.symbol : quoteSymbol }}</b>
        </label>
        <div class="new-coin-detail-percentages" :aria-label="t('newCoin.percentageOptions')">
          <button
            v-for="percentage in ([25, 50, 75, 100] as const)"
            :key="percentage"
            type="button"
            :aria-pressed="selectedPercentage === percentage"
            :disabled="!selectedAccount || (!canSubscribe && !canPurchase)"
            @click="setAmount(percentage)"
          >
            <span>{{ percentage === 100 ? t('newCoin.maximum') : t('newCoin.percentage', { value: percentage }) }}</span>
          </button>
        </div>
        <div id="new-coin-entry-summary" class="new-coin-detail-estimate">
          <span>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</span>
          <strong>
            {{ formatMoney(canSubscribe ? estimatedQuantity : paymentAmount, canSubscribe ? project.symbol : quoteSymbol) }}
            {{ canSubscribe ? project.symbol : quoteSymbol }}
          </strong>
        </div>
        <button
          class="new-coin-detail-action"
          type="button"
          :disabled="(!canSubscribe && !canPurchase) || (session.isAuthenticated && (submitting || !canSubmit))"
          :aria-busy="submitting"
          @click="requestSubmit"
        >
          <span>{{ submitting ? t('common.submitting') : actionLabel }}</span>
          <i><ArrowRight :size="19" /></i>
        </button>
        <p><Clock3 :size="11" />{{ t('newCoin.actionRiskHint') }}</p>
      </section>
    </template>
    <div v-else class="new-coin-detail-state"><PackageOpen :size="24" /><span>{{ t('newCoin.noProjects') }}</span></div>

    <Teleport to="body">
      <div v-if="reviewOpen && project && selectedAccount" class="entry-review-mask new-coin-detail-review-layer" @click.self="closeReview">
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
              <small id="entry-review-description">
                {{ canSubscribe
                  ? t('newCoin.subscribeDescription')
                  : t('newCoin.purchaseDescription', { price: formatMoney(executionPriceText, quoteSymbol), asset: quoteSymbol }) }}
              </small>
            </div>
            <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="submitting" @click="closeReview"><X :size="21" /></button>
          </header>
          <dl class="entry-review__summary">
            <div><dt>{{ t('newCoin.paymentAsset') }}</dt><dd>{{ selectedAccount.symbol }}</dd></div>
            <div><dt>{{ t(canSubscribe ? 'newCoin.subscriptionAmount' : 'newCoin.purchaseQuantity', { asset: project.symbol }) }}</dt><dd>{{ formatMoney(amountText, canSubscribe ? selectedAccount.symbol : project.symbol) }} {{ canSubscribe ? selectedAccount.symbol : project.symbol }}</dd></div>
            <div><dt>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</dt><dd>{{ formatMoney(canSubscribe ? estimatedQuantity : paymentAmount, canSubscribe ? project.symbol : selectedAccount.symbol) }} {{ canSubscribe ? project.symbol : selectedAccount.symbol }}</dd></div>
            <div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatMoney(availableText, selectedAccount.symbol) }} {{ selectedAccount.symbol }}</dd></div>
          </dl>
          <p v-if="error" class="entry-review__error" role="alert">{{ error }}</p>
          <div class="entry-review__actions">
            <button type="button" :disabled="submitting" data-dialog-cancel @click="closeReview">{{ t('common.cancel') }}</button>
            <button type="button" :disabled="submitting || !canSubmit" :aria-busy="submitting" @click="submit">
              {{ submitting ? t('common.submitting') : actionLabel }}
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </main>
</template>

<style scoped>
.new-coin-detail-pencil {
  min-height: 100dvh;
  overflow-x: clip;
  padding-bottom: env(safe-area-inset-bottom);
}

.new-coin-detail-pencil :deep(.pencil-page-header) {
  background: var(--new-coin-detail-header);
  height: 56px;
  min-height: 56px;
  padding: 6px 16px;
}

.new-coin-detail-pencil :deep(.page-header__title) {
  font-size: 21px;
  font-weight: 700;
  line-height: 30px;
}

.new-coin-detail-visual {
  background: var(--new-coin-detail-visual);
  box-sizing: border-box;
  display: grid;
  gap: 8px;
  height: 210px;
  padding: 16px;
}

.new-coin-detail-visual > header {
  align-items: center;
  display: flex;
  height: 56px;
  min-width: 0;
}

.new-coin-detail-visual > header :deep(.asset-mark) {
  border-radius: 18px;
}

.new-coin-detail-identity {
  display: grid;
  flex: 1;
  margin-left: 10px;
  min-width: 0;
}

.new-coin-detail-visual h1 {
  font-size: 22px;
  font-weight: 800;
  line-height: 27px;
  margin: 0;
}

.new-coin-detail-visual header small {
  color: var(--new-coin-detail-muted);
  font-size: 10px;
  line-height: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-visual header > b {
  align-items: center;
  background: var(--new-coin-detail-status);
  border-radius: 13px;
  color: var(--new-coin-detail-signal);
  display: inline-flex;
  font-size: 10px;
  height: 26px;
  justify-content: center;
  max-width: 92px;
  min-width: 69px;
  overflow: hidden;
  padding: 0 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-visual > p {
  color: var(--new-coin-detail-muted);
  font-size: 10px;
  line-height: 14px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-plates {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 48px;
  margin: 0;
}

.new-coin-detail-plates > div {
  background: var(--new-coin-detail-plate);
  border-radius: 14px;
  display: grid;
  align-content: center;
  min-width: 0;
  padding: 0 10px 0 35px;
  position: relative;
}

.new-coin-detail-plates dt {
  color: var(--new-coin-detail-muted);
  font-size: 9px;
  line-height: 12px;
}

.new-coin-detail-plates dt svg {
  left: 10px;
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
}

.new-coin-detail-plates dd {
  font-family: var(--font-numeric);
  font-size: 12px;
  font-weight: 750;
  line-height: 17px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-progress {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 22px;
}

.new-coin-detail-progress > span {
  background: var(--new-coin-detail-progress);
  border-radius: 4px;
  height: 8px;
  overflow: hidden;
}

.new-coin-detail-progress i {
  background: var(--new-coin-detail-signal);
  border-radius: inherit;
  display: block;
  height: 100%;
}

.new-coin-detail-progress small {
  color: var(--new-coin-detail-muted);
  font-family: var(--font-numeric);
  font-size: 10px;
}

.new-coin-detail-stages {
  box-sizing: border-box;
  height: 112px;
  padding: 14px 16px;
}

.new-coin-detail-stages > header {
  align-items: center;
  display: flex;
  font-size: 14px;
  height: 22px;
  justify-content: space-between;
  margin-bottom: 10px;
}

.new-coin-detail-stages h2 {
  font-size: 14px;
  font-weight: 700;
  line-height: 22px;
  margin: 0;
}

.new-coin-detail-stages header small {
  color: var(--new-coin-detail-signal);
  font-family: var(--font-numeric);
  font-size: 10px;
  line-height: 16px;
}

.new-coin-detail-stages ol {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 52px;
  list-style: none;
  margin: 0;
  padding: 0;
}

.new-coin-detail-stages li {
  align-items: center;
  background: transparent;
  border-radius: 14px;
  color: var(--new-coin-detail-muted);
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
}

.new-coin-detail-stages li span {
  align-items: center;
  background: var(--new-coin-detail-stage);
  border-radius: 7px;
  display: flex;
  height: 22px;
  justify-content: center;
  width: 22px;
}

.new-coin-detail-stages li strong {
  font-size: 10px;
  line-height: 14px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-stages li.active {
  background: var(--new-coin-detail-stage-active);
  color: var(--new-coin-detail-ink);
}

.new-coin-detail-stages li.completed span,
.new-coin-detail-stages li.active span {
  background: var(--new-coin-detail-status);
  color: var(--new-coin-detail-signal);
}

.new-coin-detail-rules {
  box-sizing: border-box;
  height: 104px;
  padding: 0 16px 10px;
}

.new-coin-detail-rules > header {
  align-items: center;
  display: flex;
  height: 40px;
  justify-content: space-between;
}

.new-coin-detail-rules h2 {
  font-size: 14px;
  font-weight: 700;
  line-height: 22px;
  margin: 0;
}

.new-coin-detail-rules > header svg {
  color: var(--new-coin-detail-muted);
}

.new-coin-detail-rules dl {
  background: var(--new-coin-detail-rule);
  border-radius: 16px;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 54px;
  margin: 0;
  padding: 0 10px;
}

.new-coin-detail-rules dl div {
  align-content: center;
  display: grid;
  min-width: 0;
  text-align: center;
}

.new-coin-detail-rules dl div + div {
  border-left: 1px solid var(--new-coin-detail-line);
}

.new-coin-detail-rules dt {
  color: var(--new-coin-detail-muted);
  font-size: 9px;
  line-height: 13px;
}

.new-coin-detail-rules dd {
  font-size: 10px;
  font-weight: 700;
  line-height: 16px;
  margin: 0;
  overflow: hidden;
  padding: 0 5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-entry {
  background: var(--new-coin-detail-panel);
  border-radius: 26px 26px 0 0;
  box-sizing: border-box;
  display: grid;
  gap: 11px;
  min-height: 328px;
  padding: 16px 16px calc(16px + env(safe-area-inset-bottom));
}

.new-coin-detail-entry h2 {
  font-size: 18px;
  font-weight: 750;
  height: 26px;
  line-height: 26px;
  margin: 0;
}

.new-coin-detail-balance,
.new-coin-detail-estimate {
  align-items: center;
  display: flex;
  font-size: 10px;
  height: 24px;
  justify-content: space-between;
  min-width: 0;
}

.new-coin-detail-balance span,
.new-coin-detail-estimate span {
  color: var(--new-coin-detail-muted);
}

.new-coin-detail-balance strong,
.new-coin-detail-estimate strong {
  font-family: var(--font-numeric);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-amount {
  background: var(--new-coin-detail-field);
  border: 1px solid var(--new-coin-detail-line);
  border-radius: 17px;
  box-sizing: border-box;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 56px;
  padding: 7px 14px;
  position: relative;
}

.new-coin-detail-amount:focus-within {
  border-color: var(--new-coin-detail-signal);
}

.new-coin-detail-amount > span {
  color: var(--new-coin-detail-muted);
  font-size: 9px;
  left: 14px;
  line-height: 12px;
  position: absolute;
  top: 6px;
}

.new-coin-detail-amount input {
  background: transparent;
  border: 0;
  color: var(--new-coin-detail-ink);
  font-family: var(--font-numeric);
  font-size: 20px;
  font-weight: 750;
  min-width: 0;
  outline: 0;
  padding: 11px 0 0;
}

.new-coin-detail-amount > b {
  align-self: end;
  font-size: 11px;
  line-height: 26px;
  max-width: 86px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-percentages {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 32px;
}

.new-coin-detail-percentages button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--new-coin-detail-muted);
  display: flex;
  font-size: 11px;
  height: 44px;
  justify-content: center;
  margin-top: -6px;
  min-width: 0;
  padding: 0;
}

.new-coin-detail-percentages button span {
  align-items: center;
  background: var(--new-coin-detail-stage);
  border-radius: 11px;
  display: flex;
  height: 32px;
  justify-content: center;
  overflow: hidden;
  padding: 0 5px;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.new-coin-detail-percentages button[aria-pressed='true'] span {
  background: var(--new-coin-detail-stage-active);
  color: var(--new-coin-detail-signal);
  font-weight: 700;
}

.new-coin-detail-action {
  align-items: center;
  background: var(--new-coin-detail-action);
  border: 0;
  border-radius: 26px;
  color: var(--new-coin-detail-action-ink);
  display: flex;
  height: 52px;
  justify-content: space-between;
  min-width: 0;
  padding: 0 7px 0 20px;
}

.new-coin-detail-action span {
  font-size: 14px;
  font-weight: 750;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-detail-action i {
  align-items: center;
  background: var(--new-coin-detail-signal);
  border-radius: 50%;
  color: var(--new-coin-detail-action-icon);
  display: flex;
  height: 38px;
  justify-content: center;
  width: 38px;
}

.new-coin-detail-action:disabled {
  opacity: .5;
}

.new-coin-detail-entry > p {
  align-items: center;
  color: var(--new-coin-detail-muted);
  display: flex;
  font-size: 9px;
  gap: 4px;
  line-height: 12px;
  margin: 0;
}

.new-coin-detail-feedback {
  align-items: center;
  background: var(--new-coin-detail-status);
  color: var(--new-coin-detail-signal);
  display: flex;
  font-size: 11px;
  gap: 7px;
  min-height: 36px;
  padding: 4px 16px;
}

.new-coin-detail-feedback--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.new-coin-detail-state {
  align-items: center;
  color: var(--new-coin-detail-muted);
  display: flex;
  flex-direction: column;
  gap: 10px;
  justify-content: center;
  min-height: 400px;
  padding: 16px;
  text-align: center;
}

.new-coin-detail-state button {
  align-items: center;
  background: var(--new-coin-detail-action);
  border: 0;
  border-radius: 12px;
  color: var(--new-coin-detail-action-ink);
  display: flex;
  gap: 6px;
  min-height: 44px;
  padding: 0 18px;
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
  background: var(--new-coin-detail-panel);
  border-radius: 24px 24px 0 0;
  box-sizing: border-box;
  color: var(--new-coin-detail-ink);
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
.entry-review > header small,
.entry-review__summary dt {
  color: var(--new-coin-detail-muted);
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

.entry-review__summary dd {
  font-family: var(--font-numeric);
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

.entry-review__actions button {
  background: var(--new-coin-detail-stage);
  border: 0;
  border-radius: 14px;
  color: var(--new-coin-detail-ink);
  min-height: 48px;
}

.entry-review__actions button:last-child {
  background: var(--new-coin-detail-action);
  color: var(--new-coin-detail-action-ink);
}

@media (max-width: 340px) {
  .new-coin-detail-visual,
  .new-coin-detail-stages,
  .new-coin-detail-rules,
  .new-coin-detail-entry {
    padding-left: 16px;
    padding-right: 16px;
  }

  .new-coin-detail-plates > div {
    padding-left: 29px;
    padding-right: 7px;
  }

  .new-coin-detail-plates dt svg {
    left: 7px;
  }

  .new-coin-detail-stages ol {
    gap: 3px;
  }

  .new-coin-detail-stages li strong {
    font-size: 9px;
  }
}
</style>
