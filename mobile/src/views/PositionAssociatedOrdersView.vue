<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronLeft, CircleAlert, LoaderCircle, Share2 } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchMarketPairs } from '@/api/market'
import {
  fetchMarginPosition,
  fetchMarginPositionExecutions,
  fetchMarginProducts,
  type MarginPosition,
  type MarginPositionExecution,
} from '@/api/trading'
import {
  decimalNegate,
  decimalSign,
  type DecimalText,
} from '@/core/decimal'
import { goBackOr } from '@/core/navigation'
import {
  formatMarginContractTitle,
  formatRecordDecimal,
  formatRecordSignedDecimal,
  formatTransactionRecordDisplayNo,
  latestExecutionTime,
  marginExecutionQuantity,
  marginPositionClosedInterest,
  marginPositionClosedRealizedPnl,
  marginPositionClosedQuantity,
  marginPositionOriginalQuantity,
  reconstructMarginPositionExposure,
} from '@/core/transactionRecords'
import type { MarginProduct, MarketPair } from '@/core/types'
import { currentIntlLocale } from '@/i18n'
import { useSessionStore } from '@/stores/session'

interface AssociatedRecord {
  id: string
  displayId: string
  occurredAt: number
  direction: string
  tone: 'positive' | 'negative'
  time: string
  amount: string
  quantity: string
  averagePrice: string
}

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const position = ref<MarginPosition | null>(null)
const executions = ref<MarginPositionExecution[]>([])
const products = ref<MarginProduct[]>([])
const pairs = ref<MarketPair[]>([])
const loading = ref(false)
const error = ref('')
const shareFeedback = ref('')
let requestGeneration = 0
let controller: AbortController | null = null
let shareFeedbackTimer: ReturnType<typeof setTimeout> | undefined

const positionId = computed(() => String(route.params.id || '').trim())
const product = computed(() => position.value
  ? products.value.find((item) => item.id === position.value?.productId || item.pairId === position.value?.pairId)
  : undefined)
const symbol = computed(() => {
  if (!position.value) return '--/--'
  return product.value?.symbol
    || pairs.value.find((pair) => pair.id === position.value?.pairId)?.symbol
    || t('orders.contractNumber', { id: position.value.productId })
})
const contractTitle = computed(() => position.value
  ? formatMarginContractTitle(symbol.value, t('orders.perpetual'))
  : '--')
const pair = computed(() => {
  const [base = '', quote = ''] = symbol.value.replace(/[_-]/g, '/').split('/')
  return { base, quote }
})
const marginAsset = computed(() => product.value?.marginAssetSymbol || pair.value.quote)
const positionRealizedPnl = computed(() => position.value?.realizedPnlText || null)
const closeProfit = computed(() => position.value
  ? marginPositionClosedRealizedPnl(position.value, executions.value)
  : null)
const closedQuantity = computed(() => position.value
  ? marginPositionClosedQuantity(position.value, executions.value)
  : null)
const interest = computed(() => position.value
  ? marginPositionClosedInterest(position.value, executions.value)
  : null)
const associatedRecords = computed<AssociatedRecord[]>(() => {
  if (!position.value) return []
  const openingTime = position.value.openedAt || position.value.createdAt
  const exposure = reconstructMarginPositionExposure(position.value, executions.value)
  const openingDisplayId = formatTransactionRecordDisplayNo('MO', position.value.id, openingTime)
  const opening: AssociatedRecord = {
    id: `opening-${openingDisplayId}`,
    displayId: openingDisplayId,
    occurredAt: openingTime || 0,
    direction: t(position.value.direction === 'long' ? 'associated.openLong' : 'associated.openShort'),
    tone: position.value.direction === 'long' ? 'positive' : 'negative',
    time: dateTime(openingTime, true),
    amount: amount(exposure.originalNotionalText, marginAsset.value),
    quantity: amount(
      marginPositionOriginalQuantity(position.value, executions.value),
      pair.value.base,
    ),
    averagePrice: decimal(position.value.entryPriceText),
  }
  const closing = executions.value.map((execution): AssociatedRecord => {
    const displayId = formatTransactionRecordDisplayNo('MC', execution.id, execution.createdAt)
    return {
      id: `closing-${displayId}`,
      displayId,
      occurredAt: execution.createdAt,
      direction: t(position.value?.direction === 'long' ? 'associated.closeLong' : 'associated.closeShort'),
      tone: position.value?.direction === 'long' ? 'negative' : 'positive',
      time: dateTime(execution.createdAt, true),
      amount: amount(execution.closeNotionalAmountText, marginAsset.value),
      quantity: amount(
        marginExecutionQuantity(execution, position.value?.entryPriceText),
        pair.value.base,
      ),
      averagePrice: decimal(execution.exitPriceText),
    }
  }).sort((left, right) => right.occurredAt - left.occurredAt || left.id.localeCompare(right.id))
  return [...closing, opening]
})

function decimal(value: DecimalText | null | undefined): string {
  return value ? formatRecordDecimal(value, currentIntlLocale(), 8) : '--'
}

function signed(value: DecimalText | null | undefined): string {
  return value ? formatRecordSignedDecimal(value, currentIntlLocale(), 8) : '--'
}

function amount(value: DecimalText | null | undefined, asset?: string, signedValue = false): string {
  const valueText = signedValue ? signed(value) : decimal(value)
  return valueText === '--' || !asset ? valueText : `${valueText} ${asset}`
}

function dateTime(timestamp?: number, compact = false): string {
  if (!timestamp) return '--'
  const options: Intl.DateTimeFormatOptions = {
    month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
  }
  if (!compact) options.year = 'numeric'
  return new Intl.DateTimeFormat(currentIntlLocale(), options).format(timestamp)
}

function pnlTone(value: DecimalText | null | undefined): string {
  if (!value) return 'is-muted'
  return decimalSign(value) > 0 ? 'is-positive' : decimalSign(value) < 0 ? 'is-negative' : 'is-muted'
}

async function load(): Promise<void> {
  controller?.abort()
  const currentRequest = ++requestGeneration
  const generation = session.generation
  controller = new AbortController()
  if (!session.isAuthenticated || !positionId.value) {
    position.value = null
    executions.value = []
    loading.value = false
    error.value = ''
    return
  }
  loading.value = true
  error.value = ''
  try {
    const [nextPosition, nextExecutions, nextProducts, nextPairs] = await Promise.all([
      fetchMarginPosition(positionId.value, controller.signal),
      fetchMarginPositionExecutions(positionId.value, controller.signal),
      fetchMarginProducts(),
      fetchMarketPairs(),
    ])
    if (currentRequest !== requestGeneration || generation !== session.generation || controller.signal.aborted) return
    position.value = nextPosition
    executions.value = nextExecutions
    products.value = nextProducts
    pairs.value = nextPairs
  } catch (reason) {
    if (currentRequest !== requestGeneration || controller.signal.aborted) return
    error.value = apiErrorMessage(reason, t('associated.loadFailed'))
  } finally {
    if (currentRequest === requestGeneration) loading.value = false
  }
}

async function back(): Promise<void> {
  await goBackOr(router, { name: 'orders', query: { tab: 'position-history' } })
}

function announceFeedback(message: string): void {
  if (shareFeedbackTimer) clearTimeout(shareFeedbackTimer)
  shareFeedback.value = message
  shareFeedbackTimer = setTimeout(() => {
    shareFeedback.value = ''
    shareFeedbackTimer = undefined
  }, 3_000)
}

async function share(): Promise<void> {
  if (!position.value) return
  const text = contractTitle.value
  try {
    if (typeof navigator === 'undefined') throw new Error('share unavailable')
    if (typeof navigator.share === 'function') await navigator.share({ title: text, text, url: window.location.href })
    else if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(`${text}\n${window.location.href}`)
    else throw new Error('clipboard unavailable')
    announceFeedback(t('associated.shared'))
  } catch {
    announceFeedback(t('associated.shareFailed'))
  }
}

async function copyDisplayId(displayId: string): Promise<void> {
  try {
    if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) throw new Error('clipboard unavailable')
    await navigator.clipboard.writeText(displayId)
    announceFeedback(t('associated.orderNumberCopied'))
  } catch {
    announceFeedback(t('associated.copyFailed'))
  }
}

watch([positionId, () => session.generation], load, { immediate: true })
onBeforeUnmount(() => {
  requestGeneration += 1
  controller?.abort()
  if (shareFeedbackTimer) clearTimeout(shareFeedbackTimer)
})
</script>

<template>
  <main class="page page--plain pencil-page associated-page" data-associated-orders-source="margin-position-executions">
    <header class="associated-header">
      <button type="button" :aria-label="t('common.back')" @click="back"><ChevronLeft :size="27" /></button>
      <div><h1>{{ contractTitle }}</h1><span v-if="position" :class="position.direction === 'long' ? 'is-positive' : 'is-negative'">{{ t(position.direction === 'long' ? 'orders.longShort' : 'orders.shortShort') }}</span></div>
      <button type="button" :aria-label="t('associated.share')" :disabled="!position" @click="share"><Share2 :size="24" /></button>
    </header>

    <p class="associated-feedback" role="status" aria-live="polite" aria-atomic="true">{{ shareFeedback }}</p>

    <LoginRequiredState v-if="!session.isAuthenticated" class="associated-login" :description="t('orders.loginDescription')" />
    <div v-else-if="loading" class="associated-state" role="status"><LoaderCircle :size="24" class="spin" /><span>{{ t('associated.loading') }}</span></div>
    <div v-else-if="error" class="associated-state associated-state--error" role="alert"><CircleAlert :size="25" /><strong>{{ error }}</strong><button type="button" @click="load">{{ t('common.retry') }}</button></div>
    <template v-else-if="position">
      <section class="associated-summary" :aria-label="t('associated.summary')">
        <div class="associated-summary__primary">
          <div><span>{{ t('orders.realizedPnlWithAsset', { asset: marginAsset }) }}</span><strong :class="pnlTone(positionRealizedPnl)">{{ signed(positionRealizedPnl) }}</strong></div>
          <div><span>{{ t('orders.closedQuantityWithAsset', { asset: pair.base }) }}</span><strong>{{ decimal(closedQuantity) }}</strong></div>
        </div>
        <dl>
          <div><dt>{{ t('associated.closeProfit') }}</dt><dd :class="pnlTone(closeProfit)">{{ amount(closeProfit, marginAsset, true) }}</dd></div>
          <div><dt>{{ t('associated.tradingFee') }}</dt><dd>--</dd></div>
          <div><dt>{{ t('associated.interestFee') }}</dt><dd>{{ interest ? amount(decimalNegate(interest), marginAsset, true) : '--' }}</dd></div>
          <div><dt>{{ t('orders.openedAt') }}</dt><dd>{{ dateTime(position.openedAt || position.createdAt) }}</dd></div>
          <div><dt>{{ t('orders.closedAt') }}</dt><dd>{{ dateTime(position.closedAt || latestExecutionTime(executions)) }}</dd></div>
        </dl>
      </section>

      <div class="associated-divider" aria-hidden="true" />
      <h2 class="associated-section-title">{{ t('orders.historyOrdersTab') }}</h2>
      <div class="associated-list" role="list">
        <article v-for="record in associatedRecords" :key="record.id" class="associated-record" role="listitem">
          <header class="associated-record__overview">
            <div class="associated-record__operation"><strong :class="`is-${record.tone}`">{{ record.direction }}</strong><time>{{ record.time }}</time></div>
            <strong class="associated-record__amount" :class="`is-${record.tone}`" :title="record.amount">{{ record.amount }}</strong>
          </header>
          <dl>
            <div><dt>{{ t('associated.filledQuantity') }}</dt><dd :title="record.quantity">{{ record.quantity }}</dd></div>
            <div><dt>{{ t('associated.fillPrice') }}</dt><dd :title="record.averagePrice">{{ record.averagePrice }}</dd></div>
            <div><dt>{{ t('associated.fee') }}</dt><dd>--</dd></div>
            <div><dt>{{ t('associated.orderNumber') }}</dt><dd class="associated-record__number"><button class="associated-record__copy" type="button" :aria-label="t('associated.copyOrderNumber')" :title="record.displayId" @click="copyDisplayId(record.displayId)">{{ record.displayId }}</button></dd></div>
          </dl>
        </article>
      </div>
    </template>
  </main>
</template>

<style scoped>
.associated-page {
  --associated-canvas: #fff;
  --associated-divider: #f6f8f7;
  --associated-ink: #111714;
  --associated-line: #edf1ef;
  --associated-muted: #8a948f;
  --associated-negative: #ff5878;
  --associated-positive: #0dbe7b;
  --associated-chip-negative: #ffe4ea;
  --associated-chip-positive: #ddf8eb;
  background: var(--associated-canvas);
  color: var(--associated-ink);
  min-width: 0;
  overflow-x: clip;
}
:global(html[data-theme='dark'] .associated-page) {
  --associated-canvas: #000;
  --associated-divider: #0b120e;
  --associated-ink: #f3f7f5;
  --associated-line: #17221c;
  --associated-muted: #8f9b94;
  --associated-positive: #45efae;
  --associated-chip-negative: #32161f;
  --associated-chip-positive: #103326;
}
.associated-header { align-items: center; background: var(--associated-canvas); box-sizing: border-box; display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; height: 62px; min-height: 62px; padding: 0 7px; position: sticky; top: env(safe-area-inset-top); z-index: var(--layer-sticky-header); }
.associated-header::before { background: var(--associated-canvas); bottom: 100%; content: ''; height: env(safe-area-inset-top); inset-inline: 0; position: absolute; }
.associated-header > button { background: transparent; border: 0; color: var(--associated-ink); display: grid; height: 44px; padding: 0; place-items: center; width: 44px; }
.associated-header > div { align-items: center; display: flex; gap: 8px; justify-content: center; min-width: 0; }
.associated-header h1 { font-size: 21px; font-weight: 700; line-height: 29px; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-header span { border-radius: 6px; flex: 0 0 auto; font-size: 14px; font-weight: 700; line-height: 20px; padding: 5px 7px; }
.associated-header span.is-positive { background: var(--associated-chip-positive); }
.associated-header span.is-negative { background: var(--associated-chip-negative); }
.is-positive { color: var(--associated-positive); }
.is-negative { color: var(--associated-negative); }
.is-muted { color: var(--associated-muted); }

.associated-summary { box-sizing: border-box; display: grid; gap: 14px; min-height: 267px; padding: 18px 18px 20px; }
.associated-summary__primary { display: grid; gap: 18px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.associated-summary__primary > div { display: grid; gap: 5px; min-width: 0; }
.associated-summary__primary > div:last-child { text-align: right; }
.associated-summary span, .associated-summary dt { color: var(--associated-muted); font-size: 14px; line-height: 20px; }
.associated-summary__primary strong { font-family: var(--font-geist-mono), var(--data-font); font-size: 26px; line-height: 34px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-summary dl { display: grid; gap: 10px; margin: 0; }
.associated-summary dl > div { align-items: center; display: flex; gap: 12px; justify-content: space-between; min-width: 0; }
.associated-summary dd { font-family: var(--font-geist-mono), var(--data-font); font-size: 15px; font-weight: 600; line-height: 21px; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-divider { background: var(--associated-divider); height: 8px; }
.associated-section-title { align-items: center; border-bottom: 1px solid var(--associated-line); display: flex; font-size: 18px; font-weight: 700; height: 60px; margin: 0; padding: 0 18px; }
.associated-list { min-width: 0; }
.associated-record { border-bottom: 1px solid var(--associated-line); box-sizing: border-box; display: grid; gap: 10px; height: 218px; min-height: 218px; overflow: hidden; padding: 14px 18px; }
.associated-record__overview { align-items: center; display: flex; gap: 10px; justify-content: space-between; min-width: 0; }
.associated-record__operation { align-items: center; display: flex; gap: 8px; min-width: 0; }
.associated-record__operation strong { border-radius: 6px; flex: 0 0 auto; font-size: 14px; font-weight: 700; line-height: 20px; padding: 5px 8px; }
.associated-record__operation strong.is-positive { background: var(--associated-chip-positive); }
.associated-record__operation strong.is-negative { background: var(--associated-chip-negative); }
.associated-record time { color: var(--associated-muted); font-size: 14px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-record__amount { flex: 0 1 auto; font-family: var(--font-geist-mono), var(--data-font); font-size: 16px; font-weight: 400; line-height: 22px; max-width: 50%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-record dl { display: grid; gap: 10px; margin: 0; min-width: 0; }
.associated-record dl > div { align-items: center; display: flex; gap: 12px; justify-content: space-between; min-width: 0; }
.associated-record dt { color: var(--associated-muted); font-size: 14px; line-height: 20px; }
.associated-record dd { font-family: var(--font-geist-mono), var(--data-font); font-size: 15px; font-weight: 500; line-height: 21px; margin: 0; max-width: 70%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.associated-record__number { min-width: 0; }
.associated-record__copy { background: transparent; border: 0; color: var(--associated-ink); display: block; font: inherit; max-width: 100%; min-width: 0; overflow: hidden; padding: 0; position: relative; text-overflow: ellipsis; white-space: nowrap; }
.associated-record__copy::before { content: ''; inset: -10px -4px; position: absolute; }
.associated-state { align-items: center; color: var(--associated-muted); display: flex; flex-direction: column; gap: 12px; justify-content: center; min-height: 360px; padding: 24px; text-align: center; }
.associated-state--error { color: var(--associated-negative); }
.associated-state button { background: var(--associated-divider); border: 0; border-radius: 12px; color: var(--associated-ink); min-height: 44px; padding: 0 18px; }
.associated-feedback { clip: rect(0 0 0 0); clip-path: inset(50%); height: 1px; margin: 0; overflow: hidden; position: absolute; white-space: nowrap; width: 1px; }
.associated-login { border: 0; border-radius: 0; margin: 0; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.associated-header button:focus-visible, .associated-state button:focus-visible, .associated-record__copy:focus-visible { box-shadow: inset 0 0 0 2px var(--focus-ring); outline: 0; }
@media (max-width: 340px) {
  .associated-summary { padding-inline: 14px; }
  .associated-summary__primary { gap: 10px; }
  .associated-summary__primary strong { font-size: 22px; }
  .associated-record { padding-inline: 14px; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
