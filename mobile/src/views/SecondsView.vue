<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  CircleAlert,
  History,
  LoaderCircle,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKlines } from '@/api/market'
import {
  fetchSecondsOrders,
  fetchSecondsProducts,
  openSecondsOrder,
  type SecondsCycle,
  type SecondsOrder,
  type SecondsProduct,
} from '@/api/seconds'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import {
  createBottomNavSecondsFallbackTarget,
  isBottomNavigationSecondsEntry,
} from '@/core/navigation'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import type { KlinePoint, WalletAccount } from '@/core/types'

const session = useSessionStore()
const marketStore = useMarketStore()
const router = useRouter()
const { t } = useI18n()
const products = ref<SecondsProduct[]>([])
const orders = ref<SecondsOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const sparklinePoints = ref<KlinePoint[]>([])
const selected = ref<SecondsProduct | null>(null)
const selectedCycleId = ref(0)
const direction = ref<'up' | 'down'>('up')
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')
const confirmOpen = ref(false)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
const sparklineCanvas = ref<HTMLCanvasElement | null>(null)
const ordersSection = ref<HTMLElement | null>(null)
const currentTime = ref(Date.now())
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''
let clockTimer: ReturnType<typeof setInterval> | null = null
let chartResizeObserver: ResizeObserver | null = null
let chartThemeObserver: MutationObserver | null = null
let chartRequestVersion = 0
let expiredReloadedOrderId = 0

const cycle = computed<SecondsCycle | undefined>(() => (
  selected.value?.cycles.find((item) => item.id === selectedCycleId.value)
  || selected.value?.cycles[0]
))
const account = computed(() => accounts.value.find((item) => item.assetId === selected.value?.stakeAssetId))
const selectedTicker = computed(() => marketStore.tickerFor(selected.value?.symbol || ''))
const activeOrder = computed(() => orders.value.find((order) => ['opened', 'pending', 'active'].includes(order.status.toLowerCase())) || null)
const activeTicker = computed(() => marketStore.tickerFor(activeOrder.value?.symbol || selected.value?.symbol || ''))
const activeRemainingMs = computed(() => Math.max(0, (activeOrder.value?.expiresAt || 0) - currentTime.value))
const activeProgress = computed(() => {
  const order = activeOrder.value
  if (!order || order.expiresAt <= order.createdAt) return 0
  return Math.max(0, Math.min(100, ((currentTime.value - order.createdAt) / (order.expiresAt - order.createdAt)) * 100))
})
const activeEstimatedProfit = computed(() => {
  const order = activeOrder.value
  return order ? order.stakeAmount * order.payoutRate : 0
})
const amountNumber = computed(() => Number(amount.value || 0))
const payoutRate = computed(() => cycle.value?.payoutRate || 0)
const estimatedProfit = computed(() => (
  Number.isFinite(amountNumber.value) && amountNumber.value > 0
    ? amountNumber.value * payoutRate.value
    : 0
))
const valid = computed(() => Boolean(
  cycle.value
  && Number.isFinite(amountNumber.value)
  && amountNumber.value >= cycle.value.minStake
  && (!cycle.value.maxStake || amountNumber.value <= cycle.value.maxStake)
  && amountNumber.value <= (account.value?.available || 0),
))
const quickAmounts = computed(() => {
  const activeCycle = cycle.value
  if (!activeCycle) return []
  const upperBound = Math.min(
    activeCycle.maxStake || Number.POSITIVE_INFINITY,
    account.value?.available || 0,
  )
  return [...new Set([
    activeCycle.minStake,
    activeCycle.minStake * 2,
    activeCycle.minStake * 5,
    upperBound,
  ])]
    .filter((value) => Number.isFinite(value) && value >= activeCycle.minStake && value <= upperBound)
    .slice(0, 4)
})
const quickAmountSlots = computed(() => (
  quickAmounts.value.length ? quickAmounts.value : [0, 0, 0, 0]
))
const homeFallback = createBottomNavSecondsFallbackTarget()
const preferHomeFallback = computed(() => {
  void router.currentRoute.value.fullPath
  return isBottomNavigationSecondsEntry(router.options.history.state)
})

function normalizeProductSymbol(value: string): string {
  return value.replace(/[^a-z0-9]/gi, '').toUpperCase()
}

function countdownLabel(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

const activeCountdown = computed(() => countdownLabel(activeRemainingMs.value))

async function loadSparkline(symbol: string): Promise<void> {
  const requestVersion = ++chartRequestVersion
  sparklinePoints.value = []
  if (!symbol) return
  try {
    const nextPoints = await fetchKlines(symbol, '1m')
    if (requestVersion !== chartRequestVersion || normalizeProductSymbol(selected.value?.symbol || '') !== normalizeProductSymbol(symbol)) return
    sparklinePoints.value = nextPoints.slice(-48)
  } catch {
    if (requestVersion === chartRequestVersion) sparklinePoints.value = []
  }
}

function drawSparkline(): void {
  const canvas = sparklineCanvas.value
  if (!canvas) return
  const width = Math.max(1, canvas.clientWidth)
  const height = Math.max(1, canvas.clientHeight)
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  canvas.width = Math.round(width * dpr)
  canvas.height = Math.round(height * dpr)
  const context = canvas.getContext('2d')
  if (!context) return
  context.setTransform(dpr, 0, 0, dpr, 0, 0)
  context.clearRect(0, 0, width, height)

  const styles = getComputedStyle(canvas)
  const lineColor = styles.getPropertyValue('--line').trim()
  const positiveColor = styles.getPropertyValue('--positive').trim()
  context.strokeStyle = lineColor
  context.lineWidth = 1
  for (const y of [34, 68, 102, 136]) {
    context.beginPath()
    context.moveTo(0, y + 0.5)
    context.lineTo(width, y + 0.5)
    context.stroke()
  }

  const closes = sparklinePoints.value.map((point) => point.close).filter((value) => Number.isFinite(value) && value > 0)
  if (closes.length < 2) return
  const minimum = Math.min(...closes)
  const maximum = Math.max(...closes)
  const range = maximum - minimum || 1
  const horizontalPadding = 2
  const verticalPadding = 14
  context.strokeStyle = positiveColor
  context.lineWidth = 1.5
  context.lineJoin = 'round'
  context.lineCap = 'round'
  context.beginPath()
  closes.forEach((value, index) => {
    const x = horizontalPadding + ((width - horizontalPadding * 2) * index) / (closes.length - 1)
    const y = verticalPadding + ((maximum - value) / range) * (height - verticalPadding * 2)
    if (index === 0) context.moveTo(x, y)
    else context.lineTo(x, y)
  })
  context.stroke()

  const last = closes.at(-1) || 0
  const lastY = verticalPadding + ((maximum - last) / range) * (height - verticalPadding * 2)
  context.fillStyle = positiveColor
  context.beginPath()
  context.arc(width - horizontalPadding, lastY, 4, 0, Math.PI * 2)
  context.fill()
}

function initializeSparkline(): void {
  const canvas = sparklineCanvas.value
  if (!canvas) return
  chartResizeObserver = new ResizeObserver(drawSparkline)
  chartResizeObserver.observe(canvas)
  chartThemeObserver = new MutationObserver(drawSparkline)
  chartThemeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  const stage = canvas.closest('.app-stage')
  if (stage) chartThemeObserver.observe(stage, { attributes: true, attributeFilter: ['class'] })
  drawSparkline()
}

function scrollToOrders(): void {
  ordersSection.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const currentProductId = selected.value?.id
    const productsRequest = fetchSecondsProducts()
    const [nextProducts, nextOrders, nextAccounts] = session.isAuthenticated
      ? await Promise.all([productsRequest, fetchSecondsOrders(), fetchWalletAccounts()])
      : [await productsRequest, [], []] as [SecondsProduct[], SecondsOrder[], WalletAccount[]]
    products.value = nextProducts
    orders.value = nextOrders
    accounts.value = nextAccounts
    const nextActiveOrder = nextOrders.find((order) => ['opened', 'pending', 'active'].includes(order.status.toLowerCase()))
    const nextActiveProduct = nextProducts.find((product) => normalizeProductSymbol(product.symbol) === normalizeProductSymbol(nextActiveOrder?.symbol || ''))
    selected.value = nextActiveProduct || nextProducts.find((product) => product.id === currentProductId) || nextProducts[0] || null
    if (selected.value) {
      const activeOrderCycle = nextActiveProduct?.id === selected.value.id
        ? selected.value.cycles.find((item) => item.durationSeconds === nextActiveOrder?.durationSeconds)
        : undefined
      const stillAvailable = selected.value.cycles.some((item) => item.id === selectedCycleId.value)
      if (activeOrderCycle) selectedCycleId.value = activeOrderCycle.id
      else if (!stillAvailable) selectedCycleId.value = selected.value.cycles[0]?.id || 0
      if (!amount.value) amount.value = String(cycle.value?.minStake || '')
      void loadSparkline(selected.value.symbol)
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.loadFailed'))
  } finally {
    loading.value = false
  }
}

function selectProduct(product: SecondsProduct): void {
  selected.value = product
  selectedCycleId.value = product.cycles[0]?.id || 0
  direction.value = 'up'
  amount.value = String(product.cycles[0]?.minStake || '')
  error.value = ''
  success.value = ''
  void loadSparkline(product.symbol)
}

function selectProductFromEvent(event: Event): void {
  const productId = Number((event.target as HTMLSelectElement).value)
  const product = products.value.find((item) => item.id === productId)
  if (product) selectProduct(product)
}

function selectCycle(cycleId: number): void {
  selectedCycleId.value = cycleId
  amount.value = String(cycle.value?.minStake || '')
  error.value = ''
  success.value = ''
}

function setDirection(nextDirection: 'up' | 'down'): void {
  direction.value = nextDirection
  error.value = ''
  success.value = ''
}

function setAmount(value: string | number): void {
  amount.value = String(value)
  error.value = ''
  success.value = ''
}

function reviewOrder(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/seconds' } })
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  error.value = ''
  confirmOpen.value = true
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
}

async function submit(): Promise<void> {
  if (!session.isAuthenticated) {
    error.value = t('seconds.loginDescription')
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    const openedOrder = await openSecondsOrder({
      productId: selected.value.id,
      durationSeconds: cycle.value.durationSeconds,
      direction: direction.value,
      stakeAmount: amountNumber.value,
    })
    orders.value = [openedOrder, ...orders.value.filter((order) => order.id !== openedOrder.id)]
    amount.value = ''
    success.value = t('seconds.created')
    confirmOpen.value = false
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.orderFailed'))
  } finally {
    submitting.value = false
  }
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    opened: 'seconds.statusActive',
    pending: 'seconds.statusPending',
    active: 'seconds.statusActive',
    won: 'seconds.statusWon',
    lost: 'seconds.statusLost',
    settled: 'seconds.statusSettled',
    cancelled: 'seconds.statusCancelled',
    canceled: 'seconds.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function orderStatusLabel(order: SecondsOrder): string {
  const result = order.result?.toLowerCase()
  if (result === 'win') return t('seconds.statusWon')
  if (result === 'loss') return t('seconds.statusLost')
  return statusLabel(order.status)
}

function orderStatusTone(order: SecondsOrder): string {
  const result = order.result?.toLowerCase()
  if (result === 'win') return 'is-positive'
  if (result === 'loss') return 'is-negative'
  return statusTone(order.status)
}

function statusTone(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'won' || normalized === 'settled') return 'is-positive'
  if (normalized === 'lost' || normalized === 'cancelled' || normalized === 'canceled') return 'is-negative'
  return 'is-pending'
}

function highestRate(product: SecondsProduct): string {
  const highest = Math.max(0, ...product.cycles.map((item) => item.payoutRate * 100))
  return highest.toFixed(0)
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeConfirm()
    return
  }
  if (event.key !== 'Tab' || !confirmDialog.value) return
  const focusable = Array.from(confirmDialog.value.querySelectorAll<HTMLElement>(
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

watch(confirmOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : reviewButton.value
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    confirmDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
    return
  }
  document.body.style.overflow = previousBodyOverflow
  await nextTick()
  returnFocus?.focus()
  returnFocus = null
})

watch(sparklinePoints, async () => {
  await nextTick()
  drawSparkline()
})

onMounted(async () => {
  await nextTick()
  initializeSparkline()
  clockTimer = setInterval(() => {
    currentTime.value = Date.now()
    const order = activeOrder.value
    if (order && activeRemainingMs.value <= 0 && expiredReloadedOrderId !== order.id) {
      expiredReloadedOrderId = order.id
      void load()
    }
  }, 1000)
  void Promise.all([load(), marketStore.refresh()])
})

onBeforeUnmount(() => {
  chartRequestVersion += 1
  if (clockTimer) clearInterval(clockTimer)
  chartResizeObserver?.disconnect()
  chartThemeObserver?.disconnect()
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main
    class="page page--plain seconds-page"
    data-pencil-source="VL8er g9agt Lpt6q WxeB8"
    data-instrument-hero="pair-price"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('seconds.scene')"
      :fallback="homeFallback"
      :pencil="true"
      :prefer-fallback="preferHomeFallback"
      :title="selected?.symbol || t('seconds.title')"
      :subtitle="t('seconds.context')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('seconds.myOrders')" @click="scrollToOrders">
          <History :size="18" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <label class="field seconds-pair-field">
      <span class="sr-only">{{ t('marketDetail.market') }}</span>
      <span class="seconds-select-shell">
        <select
          :value="selected?.id || ''"
          :disabled="loading || !products.length"
          :aria-label="t('marketDetail.market')"
          @change="selectProductFromEvent"
        >
          <option v-if="!products.length" value="">{{ loading ? t('seconds.loading') : t('seconds.noProducts') }}</option>
          <option v-for="product in products" :key="product.id" :value="product.id">
            {{ product.symbol }}
          </option>
        </select>
        <small v-if="selected" :data-highest-rate="highestRate(selected)">{{ t('seconds.title') }}</small>
      </span>
    </label>

    <div class="page-content seconds-content">
      <section
        class="seconds-workspace"
        data-seconds-workspace="live"
        :data-seconds-state="activeOrder ? 'active' : 'default'"
        :class="{ 'seconds-guest': !session.isAuthenticated }"
      >
        <section
          class="seconds-market-board"
          :data-seconds-market="selected ? 'live' : loading ? 'loading' : 'empty'"
          :aria-busy="loading || marketStore.loading"
        >
          <div class="seconds-round-row">
            <i aria-hidden="true" />
            <span>
              {{ t('seconds.currentRound') }}
              <b v-if="activeOrder" class="numeric">#{{ activeOrder.id }}</b>
              <b v-else>{{ selected?.status || (selectedTicker ? t('common.liveData') : t('common.marketUnavailable')) }}</b>
            </span>
          </div>

          <div class="seconds-price-row">
            <strong class="numeric">{{ selectedTicker ? formatPrice(selectedTicker.lastPrice) : '--' }}</strong>
            <span class="numeric">
              {{ activeOrder ? activeCountdown : cycle ? t('seconds.duration', { seconds: cycle.durationSeconds }) : '--' }}
              · {{ cycle ? `${(payoutRate * 100).toFixed(2)}%` : '--' }}
            </span>
          </div>

          <div class="seconds-micro-chart" :data-chart-state="sparklinePoints.length ? 'ready' : 'empty'">
            <canvas ref="sparklineCanvas" :aria-label="t('seconds.referencePrice')" />
            <span v-if="!sparklinePoints.length" role="status">
              {{ loading ? t('common.loading') : t('common.marketUnavailable') }}
            </span>
          </div>
        </section>

        <section v-if="activeOrder" class="seconds-active-order" data-active-order="real">
          <header>
            <span :class="activeOrder.direction">
              <ArrowUp v-if="activeOrder.direction === 'up'" :size="12" aria-hidden="true" />
              <ArrowDown v-else :size="12" aria-hidden="true" />
              {{ t(activeOrder.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
            </span>
            <b class="numeric">{{ t('seconds.duration', { seconds: activeOrder.durationSeconds }) }}</b>
            <strong class="numeric">{{ statusLabel(activeOrder.status) }} {{ activeCountdown }}</strong>
          </header>
          <div class="seconds-active-progress" aria-hidden="true">
            <i :style="{ width: `${activeProgress}%` }" />
          </div>
          <dl>
            <div>
              <dt>{{ t('orders.entryPrice') }}</dt>
              <dd class="numeric">{{ activeOrder.entryPrice !== undefined ? formatPrice(activeOrder.entryPrice) : '--' }}</dd>
            </div>
            <div>
              <dt>{{ t('marketDetail.latestPrice') }}</dt>
              <dd class="positive numeric">{{ activeTicker ? formatPrice(activeTicker.lastPrice) : '--' }}</dd>
            </div>
            <div>
              <dt>{{ t('seconds.stakeAmount') }}</dt>
              <dd class="numeric">{{ formatAmount(activeOrder.stakeAmount) }} {{ activeOrder.stakeAssetSymbol }}</dd>
            </div>
            <div>
              <dt>{{ t('seconds.estimatedProfit') }}</dt>
              <dd class="positive numeric">
                +{{ formatAmount(activeEstimatedProfit) }} {{ activeOrder.stakeAssetSymbol }}
              </dd>
            </div>
          </dl>
        </section>

        <section
          class="instrument-plate seconds-order-console"
          data-instrument-plate="market-and-order"
        >
          <section class="seconds-control-group" aria-labelledby="seconds-direction-label">
            <div class="seconds-control-label">
              <span id="seconds-direction-label">{{ t('seconds.direction') }}</span>
            </div>
            <div class="seconds-direction-grid" role="group" :aria-label="t('seconds.direction')">
              <button
                type="button"
                class="up"
                :class="{ active: direction === 'up' }"
                :aria-pressed="direction === 'up'"
                :disabled="loading || !selected || Boolean(activeOrder)"
                @click="setDirection('up')"
              >
                <ArrowUp :size="16" aria-hidden="true" />
                <span>{{ t('seconds.bullish') }}</span>
              </button>
              <button
                type="button"
                class="down"
                :class="{ active: direction === 'down' }"
                :aria-pressed="direction === 'down'"
                :disabled="loading || !selected || Boolean(activeOrder)"
                @click="setDirection('down')"
              >
                <ArrowDown :size="16" aria-hidden="true" />
                <span>{{ t('seconds.bearish') }}</span>
              </button>
            </div>
          </section>

          <section class="seconds-control-group" aria-labelledby="seconds-duration-label">
            <div class="seconds-control-label">
              <span id="seconds-duration-label">{{ t('seconds.term') }}</span>
            </div>
            <div class="seconds-duration-grid" role="group" :aria-label="t('seconds.term')">
              <template v-if="selected?.cycles.length">
                <button
                  v-for="item in selected.cycles"
                  :key="item.id"
                  type="button"
                  :class="{ active: cycle?.id === item.id }"
                  :aria-pressed="cycle?.id === item.id"
                  :disabled="Boolean(activeOrder)"
                  @click="selectCycle(item.id)"
                >
                  <span>{{ t('seconds.duration', { seconds: item.durationSeconds }) }}</span>
                </button>
              </template>
              <template v-else>
                <button v-for="slot in 4" :key="slot" type="button" disabled><span>--</span></button>
              </template>
            </div>
          </section>

          <label
            class="field seconds-amount-field"
            :data-field-state="amount && !valid ? 'invalid' : amount && valid ? 'complete' : 'idle'"
          >
            <span>{{ t('seconds.stakeAmount') }}</span>
            <div>
              <input
                v-model="amount"
                class="numeric"
                inputmode="decimal"
                :disabled="loading || !selected || Boolean(activeOrder)"
                :aria-invalid="Boolean(amount) && !valid"
                @input="setAmount(amount)"
              />
              <b>{{ selected?.stakeAssetSymbol || '--' }}</b>
            </div>
          </label>

          <div class="seconds-amount-presets" role="group" :aria-label="t('seconds.stakeAmount')">
            <button
              v-for="(value, index) in quickAmountSlots"
              :key="`${value}-${index}`"
              type="button"
              :aria-pressed="value > 0 && amountNumber === value"
              :disabled="loading || !selected || Boolean(activeOrder) || value <= 0"
              @click="setAmount(value)"
            >
              {{ value > 0 ? formatAmount(value) : '--' }}
            </button>
          </div>

          <dl class="seconds-order-summary">
            <div>
              <dt>{{ t('swap.spotWalletNote') }}</dt>
              <dd class="numeric">
                {{ selected && session.isAuthenticated && account
                  ? `${formatAmount(account.available)} ${selected.stakeAssetSymbol}`
                  : '--' }}
              </dd>
            </div>
          </dl>

          <div class="seconds-feedback" aria-live="polite">
            <div v-if="error" class="seconds-message seconds-message--error" role="alert">
              <CircleAlert :size="16" aria-hidden="true" />
              <span>{{ error }}</span>
              <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="16" /></button>
            </div>
            <div v-else-if="success" class="seconds-message seconds-message--success" data-session-feedback="created" role="status">
              <CheckCircle2 :size="16" aria-hidden="true" />
              <span>{{ success }}</span>
            </div>
            <span v-else-if="loading"><LoaderCircle :size="15" class="spin" />{{ t('seconds.loading') }}</span>
            <span v-else-if="!session.isAuthenticated">{{ t('seconds.loginDescription') }}</span>
          </div>

          <button
            ref="reviewButton"
            class="button button--primary button--full seconds-submit"
            type="button"
            :disabled="submitting || loading || !selected || Boolean(activeOrder)"
            @click="reviewOrder"
          >
            {{ activeOrder ? `${statusLabel(activeOrder.status)} ${activeCountdown}` : t('seconds.confirmOrder') }}
          </button>

          <p class="seconds-risk-note">
            <CircleAlert :size="14" aria-hidden="true" />
            <span>{{ t('seconds.introDescription') }}</span>
          </p>
        </section>

        <section ref="ordersSection" class="seconds-session-records seconds-orders" :aria-label="t('seconds.myOrders')">
          <h2 class="group-title">{{ t('seconds.myOrders') }} · {{ session.isAuthenticated ? orders.length : '--' }}</h2>
          <template v-if="orders.length">
            <article v-for="order in orders.slice(0, 3)" :key="order.id">
              <div>
                <strong>{{ order.symbol }} · {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</strong>
                <span :class="orderStatusTone(order)">{{ orderStatusLabel(order) }}</span>
              </div>
              <p>
                {{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }} ·
                {{ t('seconds.duration', { seconds: order.durationSeconds }) }} ·
                {{ formatDateTime(order.createdAt) }}
              </p>
            </article>
          </template>
          <p v-else>{{ session.isAuthenticated ? t('seconds.noOrders') : t('seconds.loginDescription') }}</p>
        </section>
      </section>
    </div>

    <div v-if="confirmOpen && selected && cycle" class="confirmation-layer seconds-mask" @click.self="closeConfirm">
      <section
        ref="confirmDialog"
        class="confirmation-sheet seconds-dialog"
        role="dialog"
        aria-modal="true"
        :aria-busy="submitting"
        aria-labelledby="seconds-confirm-title"
        aria-describedby="seconds-confirm-summary"
        tabindex="-1"
        @keydown="trapDialogFocus"
      >
        <header>
          <span class="confirmation-icon"><CheckCircle2 :size="20" /></span>
          <div>
            <strong id="seconds-confirm-title">{{ t('seconds.confirmOrder') }}</strong>
            <small>{{ selected.symbol }} · {{ t('seconds.settledIn', { asset: selected.stakeAssetSymbol }) }}</small>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="submitting"
            data-dialog-cancel
            @click="closeConfirm"
          >
            <X :size="21" />
          </button>
        </header>

        <p id="seconds-confirm-summary">
          {{ selected.symbol }} · {{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }} ·
          {{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}
        </p>

        <dl class="confirmation-detail">
          <div><dt>{{ t('seconds.direction') }}</dt><dd>{{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</dd></div>
          <div><dt>{{ t('seconds.term') }}</dt><dd>{{ t('seconds.duration', { seconds: cycle.durationSeconds }) }}</dd></div>
          <div><dt>{{ t('seconds.stakeAmount') }}</dt><dd>{{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}</dd></div>
          <div>
            <dt>{{ t('seconds.payoutRate') }}</dt>
            <dd>{{ (cycle.payoutRate * 100).toFixed(2) }}% · +{{ formatAmount(estimatedProfit) }} {{ selected.stakeAssetSymbol }}</dd>
          </div>
          <div><dt>{{ t('marketDetail.latestPrice') }}</dt><dd>{{ selectedTicker ? formatPrice(selectedTicker.lastPrice) : '--' }}</dd></div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="confirmation-actions dialog-actions">
          <button type="button" class="button button--secondary" :disabled="submitting" @click="closeConfirm">
            {{ t('common.cancel') }}
          </button>
          <button
            type="button"
            class="button button--primary confirmation-primary"
            :disabled="submitting"
            :aria-busy="submitting"
            @click="submit"
          >
            {{ submitting ? t('common.submitting') : t('seconds.confirmOrder') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.seconds-page {
  background: var(--page);
  color: var(--text);
  min-width: 0;
  overflow-x: clip;
  position: relative;
}

.seconds-content {
  min-width: 0;
  padding: 0 0 calc(24px + env(safe-area-inset-bottom));
}

.seconds-workspace,
.seconds-market-board,
.seconds-order-console,
.seconds-session-records {
  min-width: 0;
}

.seconds-workspace {
  display: grid;
  gap: 0;
  width: 100%;
}

.seconds-pair-field {
  display: grid;
  left: 72px;
  min-width: 0;
  position: absolute;
  right: 72px;
  top: 4px;
  z-index: calc(var(--layer-sticky-header) + 1);
}

.seconds-select-shell {
  align-items: center;
  background: var(--surface);
  border: 0;
  border-radius: 0;
  display: grid;
  gap: 6px;
  grid-template-columns: minmax(0, auto) auto;
  height: 52px;
  justify-content: center;
  min-height: 52px;
  min-width: 0;
  padding: 0;
}

.seconds-select-shell:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.seconds-select-shell select {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--text);
  font-size: 15px;
  font-weight: 750;
  height: 52px;
  min-height: 52px;
  min-width: 0;
  outline: 0;
  padding: 0;
  text-align: right;
  width: auto;
}

.seconds-select-shell small {
  background: var(--positive-soft);
  border-radius: 50%;
  color: var(--positive);
  font-size: 9px;
  font-weight: 650;
  line-height: 20px;
  padding: 0 7px;
  white-space: nowrap;
}

.seconds-select-shell select:focus,
.seconds-select-shell select:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.seconds-select-shell:has(select:disabled) {
  opacity: .64;
}

.seconds-market-board {
  background: var(--page);
  border: 0;
  display: grid;
  gap: 6px;
  overflow: hidden;
  padding: 4px 20px 0;
}

.seconds-round-row {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  gap: 6px;
  line-height: 16px;
  min-height: 16px;
}

.seconds-round-row i {
  background: var(--positive);
  border-radius: 50%;
  flex: 0 0 6px;
  height: 6px;
  width: 6px;
}

.seconds-round-row b {
  color: var(--text);
  font-weight: 650;
}

.seconds-price-row {
  align-items: baseline;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-price-row strong {
  color: var(--text);
  font-size: 34px;
  font-weight: 750;
  letter-spacing: -.8px;
  line-height: 42px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.seconds-price-row span {
  color: var(--positive);
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 650;
  line-height: 16px;
  text-align: right;
}

.seconds-micro-chart {
  height: 170px;
  min-width: 0;
  position: relative;
}

.seconds-micro-chart canvas {
  display: block;
  height: 170px;
  width: 100%;
}

.seconds-micro-chart > span {
  color: var(--muted);
  font-size: 11px;
  left: 50%;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  white-space: nowrap;
}

.seconds-active-order {
  background: var(--surface);
  border: 1px solid var(--positive);
  border-radius: 14px;
  display: grid;
  gap: 10px;
  margin: 12px 20px 0;
  min-width: 0;
  padding: 12px 14px;
}

.seconds-active-order header {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.seconds-active-order header > span {
  align-items: center;
  background: var(--positive);
  border-radius: 11px;
  color: var(--on-accent);
  display: inline-flex;
  font-size: 10px;
  font-weight: 750;
  gap: 4px;
  height: 22px;
  padding: 0 10px;
}

.seconds-active-order header > span.down {
  background: var(--negative);
}

.seconds-active-order header b,
.seconds-active-order header strong {
  font-size: 10px;
}

.seconds-active-order header strong {
  color: var(--positive);
  margin-left: auto;
  text-align: right;
}

.seconds-active-progress {
  background: var(--positive-soft);
  border-radius: 3px;
  height: 6px;
  overflow: hidden;
}

.seconds-active-progress i {
  background: var(--positive);
  border-radius: inherit;
  display: block;
  height: 100%;
  transition: width .2s linear;
}

.seconds-active-order dl {
  display: grid;
  gap: 6px;
  margin: 0;
}

.seconds-active-order dl > div {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-active-order dt,
.seconds-active-order dd {
  font-size: 10px;
  line-height: 15px;
  margin: 0;
}

.seconds-active-order dt {
  color: var(--muted);
}

.seconds-active-order dd {
  color: var(--text);
  font-weight: 650;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-active-order dd.positive {
  color: var(--positive);
}

.seconds-order-console {
  background: var(--page);
  border: 0;
  border-radius: 0;
  box-shadow: none;
  display: grid;
  gap: 12px;
  padding: 12px 20px 20px;
}

.seconds-control-group,
.seconds-amount-field {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.seconds-control-label,
.seconds-amount-field > span {
  color: var(--text);
  font-size: 11px;
  font-weight: 650;
  line-height: 16px;
}

.seconds-direction-grid,
.seconds-duration-grid {
  display: grid;
  min-width: 0;
}

.seconds-direction-grid {
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.seconds-direction-grid button {
  min-height: 52px;
  align-items: center;
  background: var(--positive);
  border: 1px solid transparent;
  border-radius: 12px;
  color: var(--on-accent);
  display: inline-flex;
  font-size: 14px;
  font-weight: 750;
  gap: 6px;
  justify-content: center;
  min-width: 0;
}

.seconds-direction-grid button.down {
  background: var(--negative-soft);
  color: var(--negative);
}

.seconds-direction-grid button:not(.active) {
  opacity: .72;
}

.seconds-direction-grid button:disabled {
  cursor: default;
}

.seconds-duration-grid {
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.seconds-duration-grid button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 18px;
  color: var(--text);
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  height: 36px;
  justify-content: center;
  min-height: 36px;
  min-width: 0;
  padding: 0 6px;
}

.seconds-duration-grid button.active {
  background: var(--positive-soft);
  border-color: transparent;
  color: var(--positive);
}

.seconds-amount-field {
  gap: 0;
  height: 52px;
  min-height: 52px;
  padding: 0;
  position: relative;
}

.seconds-amount-field > span {
  left: 0;
  position: absolute;
  top: 2px;
  z-index: 1;
}

.seconds-page .seconds-order-console .seconds-amount-field > div {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--line);
  box-shadow: none;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 52px;
  min-height: 52px;
  min-width: 0;
  padding: 13px 0 0;
}

.seconds-page .seconds-order-console .seconds-amount-field:focus-within > div {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.seconds-page .seconds-order-console .seconds-amount-field[data-field-state="invalid"] > div {
  border-color: var(--negative);
}

.seconds-page .seconds-order-console .seconds-amount-field input {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--text);
  font-size: 22px;
  font-weight: 750;
  height: 38px;
  min-height: 38px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.seconds-page .seconds-order-console .seconds-amount-field input:focus,
.seconds-page .seconds-order-console .seconds-amount-field input:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.seconds-amount-field b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.seconds-amount-presets {
  display: none;
}

.seconds-order-summary {
  margin: -4px 0 0;
  min-width: 0;
}

.seconds-order-summary > div {
  align-items: baseline;
  display: flex;
  gap: 6px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-order-summary dt,
.seconds-order-summary dd {
  color: var(--muted);
  font-size: 10px;
  line-height: 16px;
  margin: 0;
}

.seconds-order-summary dd {
  color: var(--text);
  font-weight: 650;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-feedback {
  display: grid;
  min-height: 0;
  min-width: 0;
}

.seconds-feedback > span {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 7px;
  line-height: 16px;
}

.seconds-message {
  align-items: center;
  border: 1px solid currentColor;
  border-radius: 10px;
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 16px;
  min-height: 44px;
  min-width: 0;
  padding: 3px 4px 3px 11px;
}

.seconds-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.seconds-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.seconds-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 36px;
  min-width: 36px;
  place-items: center;
}

.seconds-submit {
  border-radius: 26px;
  min-height: 52px;
  width: 100%;
}

.seconds-submit:disabled {
  background: var(--positive-soft);
  color: var(--positive);
  opacity: 1;
}

.seconds-risk-note {
  align-items: flex-start;
  background: var(--negative-soft);
  border-radius: 10px;
  color: var(--negative);
  display: flex;
  font-size: 11px;
  font-weight: 500;
  gap: 8px;
  line-height: 16px;
  margin: 0;
  padding: 10px 12px;
}

.seconds-risk-note svg {
  flex: 0 0 auto;
  margin-top: 1px;
}

.seconds-session-records {
  border-top: 1px solid var(--line);
  display: grid;
  margin-top: 72px;
  padding: 0 20px;
}

.seconds-session-records h2 {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  font-size: 13px;
  margin: 0;
  min-height: 52px;
  overflow-wrap: anywhere;
}

.seconds-session-records article {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 7px;
  min-width: 0;
  padding: 13px 0;
}

.seconds-session-records article > div {
  display: flex;
  gap: 9px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-session-records article strong,
.seconds-session-records article span {
  font-size: 10px;
  overflow-wrap: anywhere;
}

.seconds-session-records article span {
  color: var(--positive);
  text-align: right;
}

.seconds-session-records article span.is-negative {
  color: var(--negative);
}

.seconds-session-records article span.is-pending {
  color: var(--accent);
}

.seconds-session-records article p,
.seconds-session-records > p {
  color: var(--muted);
  font-size: 9px;
  line-height: 15px;
  margin: 0;
  overflow-wrap: anywhere;
}

.seconds-session-records > p {
  min-height: 72px;
  padding: 16px 0;
}

.seconds-duration-grid button:focus-visible,
.seconds-direction-grid button:focus-visible,
.seconds-submit:focus-visible,
.seconds-message button:focus-visible,
.seconds-dialog button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

.seconds-page .seconds-mask {
  align-items: flex-end;
  background: var(--overlay);
  display: flex;
  inset: 0;
  justify-content: center;
  max-width: 100%;
  padding:
    max(16px, env(safe-area-inset-top))
    16px
    max(16px, env(safe-area-inset-bottom));
  position: fixed;
  width: 100%;
  z-index: 90;
}

.seconds-dialog {
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 15px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.seconds-dialog header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  min-width: 0;
}

.seconds-dialog header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.seconds-dialog header strong {
  font-size: 18px;
}

.seconds-dialog header small {
  color: var(--muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.seconds-dialog header .icon-button {
  height: 44px;
  min-height: 44px;
  min-width: 44px;
  width: 44px;
}

.seconds-dialog > p {
  border-block: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  overflow-wrap: anywhere;
  padding: 12px 0;
}

.seconds-dialog dl {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.seconds-dialog dl > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.seconds-dialog dl > div:last-child {
  border-bottom: 0;
}

.seconds-dialog dt,
.seconds-dialog dd {
  font-size: 12px;
  margin: 0;
}

.seconds-dialog dt {
  color: var(--muted);
}

.seconds-dialog dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  max-width: 64%;
  overflow-wrap: anywhere;
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
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 340px) {
  .seconds-pair-field {
    left: 64px;
    right: 64px;
  }

  .seconds-market-board,
  .seconds-order-console,
  .seconds-session-records {
    padding-inline: 16px;
  }

  .seconds-active-order {
    margin-inline: 16px;
  }

  .seconds-price-row strong {
    font-size: 30px;
  }

  .seconds-duration-grid {
    gap: 6px;
  }

  .seconds-duration-grid button {
    font-size: 10px;
    padding-inline: 3px;
  }

  .dialog-actions {
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .seconds-page *,
  .seconds-page *::before,
  .seconds-page *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }

  .seconds-page button:active {
    transform: none;
  }

  .spin {
    animation: none;
  }
}
</style>
