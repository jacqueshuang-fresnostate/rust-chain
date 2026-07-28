<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ExternalLink, Landmark, ReceiptText } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { createQuickRechargeOrder, fetchQuickRechargeConfig, fetchQuickRechargeOrders, type QuickRechargeConfig, type QuickRechargeOrder } from '@/api/wallet'
import { formatAmount, formatDateTime, formatFiat } from '@/core/format'
import { detectClientPlatform } from '@/core/platform'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const config = ref<QuickRechargeConfig | null>(null)
const orders = ref<QuickRechargeOrder[]>([])
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const submittedOrder = ref<QuickRechargeOrder | null>(null)
const validationAttempted = ref(false)

const platformTarget = computed<'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web'>(() => {
  return detectClientPlatform()
})

const numericAmount = computed(() => Number(amount.value || 0))
const amountValid = computed(() => {
  if (!config.value || !Number.isFinite(numericAmount.value)) return false
  return numericAmount.value >= config.value.minAmount && (!config.value.maxAmount || numericAmount.value <= config.value.maxAmount)
})
const amountInvalid = computed(() => validationAttempted.value && !amountValid.value)

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextConfig, nextOrders] = await Promise.all([fetchQuickRechargeConfig(), fetchQuickRechargeOrders()])
    config.value = nextConfig
    orders.value = nextOrders
    if (!amount.value && nextConfig.minAmount > 0) amount.value = String(nextConfig.minAmount)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('quickRecharge.unavailable'))
  } finally {
    loading.value = false
  }
}

function setAmount(value: number): void {
  amount.value = String(value)
}

async function submit(): Promise<void> {
  error.value = ''
  submittedOrder.value = null
  validationAttempted.value = true
  if (!config.value || !amountValid.value) {
    error.value = t('quickRecharge.invalidAmount')
    return
  }
  submitting.value = true
  try {
    const order = await createQuickRechargeOrder(numericAmount.value, platformTarget.value)
    submittedOrder.value = order
    orders.value = [order, ...orders.value.filter((item) => item.id !== order.id)]
    validationAttempted.value = false
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('quickRecharge.createFailed'))
  } finally {
    submitting.value = false
  }
}

function continuePayment(): void {
  if (!submittedOrder.value?.paymentUrl) return
  window.location.assign(submittedOrder.value.paymentUrl)
}

function orderStatusTone(status: string): string {
  const normalized = status.toLowerCase()
  if (['completed', 'confirmed', 'paid', 'success', 'succeeded'].includes(normalized)) return 'is-positive'
  if (['canceled', 'cancelled', 'expired', 'failed', 'rejected'].includes(normalized)) return 'is-negative'
  return 'is-pending'
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('quickRecharge.title')" />
    <div class="page-content recharge-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('quickRecharge.loginDescription')" />
      <template v-else>
        <p v-if="error" id="quick-recharge-error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="recharge-loading" role="status">
          <span class="recharge-loading__icon"><Landmark :size="22" aria-hidden="true" /></span>
          <span>{{ t('quickRecharge.loading') }}</span>
        </div>
        <template v-else-if="config">
          <section class="recharge-hero">
            <span><Landmark :size="22" aria-hidden="true" /></span>
            <div>
              <strong>{{ t('quickRecharge.hero', { token: config.token }) }}</strong>
              <p>{{ t('quickRecharge.heroDescription') }}</p>
            </div>
          </section>
          <template v-if="config.enabled">
            <form class="recharge-form" @submit.prevent="submit">
              <label class="recharge-amount" :class="{ 'is-invalid': amountInvalid }">
                <span>{{ t('quickRecharge.paymentAmount') }}</span>
                <span class="recharge-amount__control">
                  <input
                    v-model="amount"
                    inputmode="decimal"
                    :aria-invalid="amountInvalid"
                    :aria-describedby="amountInvalid ? 'quick-recharge-error' : undefined"
                  />
                  <b>{{ config.currency }}</b>
                </span>
              </label>
              <div class="quick-values">
                <button
                  v-for="value in [config.minAmount, config.minAmount * 2, config.minAmount * 5]"
                  :key="value"
                  type="button"
                  :aria-pressed="numericAmount === value"
                  :class="{ 'is-active': numericAmount === value }"
                  @click="setAmount(value)"
                >
                  {{ formatFiat(value, config.currency) }}
                </button>
              </div>
              <dl>
                <div><dt>{{ t('quickRecharge.receivedAsset') }}</dt><dd>{{ config.token }}</dd></div>
                <div><dt>{{ t('quickRecharge.network') }}</dt><dd>{{ config.network || t('quickRecharge.providerNetwork') }}</dd></div>
                <div><dt>{{ t('quickRecharge.amountRange') }}</dt><dd class="numeric">{{ formatFiat(config.minAmount, config.currency) }}<span v-if="config.maxAmount"> - {{ formatFiat(config.maxAmount, config.currency) }}</span></dd></div>
              </dl>
              <button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('quickRecharge.creating') : t('quickRecharge.buy', { token: config.token }) }}</button>
            </form>
            <section v-if="submittedOrder" class="order-result" aria-live="polite">
              <div><ReceiptText :size="20" aria-hidden="true" /><span>{{ t('quickRecharge.order', { id: submittedOrder.orderId }) }}</span></div>
              <strong class="numeric">{{ formatAmount(submittedOrder.actualAmount || submittedOrder.fiatAmount) }} {{ submittedOrder.actualAmount ? submittedOrder.token : submittedOrder.currency }}</strong>
              <button v-if="submittedOrder.paymentUrl" class="button button--secondary button--full" type="button" @click="continuePayment">
                {{ t('quickRecharge.continuePayment') }}
                <ExternalLink :size="16" aria-hidden="true" />
              </button>
              <p v-else>{{ t('quickRecharge.paymentPreparing') }}</p>
            </section>
          </template>
          <p v-else class="surface-note">{{ t('quickRecharge.disabled') }}</p>
          <section class="history">
            <div class="section-heading"><span>{{ t('quickRecharge.recentOrders') }}</span></div>
            <div v-if="orders.length" class="history-list" role="list">
              <article v-for="order in orders" :key="order.id" class="history-row" role="listitem">
                <div>
                  <span class="history-row__identity">
                    <strong>{{ order.token }}</strong>
                    <b class="history-row__status" :class="orderStatusTone(order.status)">{{ order.status }}</b>
                  </span>
                  <small>{{ formatDateTime(order.createdAt) }}</small>
                </div>
                <span>
                  <b class="numeric">{{ formatFiat(order.fiatAmount, order.currency) }}</b>
                  <small>{{ order.network || t('quickRecharge.quickPayment') }}</small>
                </span>
              </article>
            </div>
            <p v-else class="empty-state">{{ t('quickRecharge.empty') }}</p>
          </section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.recharge-page {
  display: grid;
  gap: 18px;
  padding-bottom: calc(38px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.recharge-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.recharge-loading__icon {
  animation: pulse 1.1s ease-in-out infinite alternate;
  color: var(--accent);
  display: grid;
  place-items: center;
}

.recharge-hero {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 13px;
  padding: 2px 0 18px;
}

.recharge-hero > span {
  align-items: center;
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: 50%;
  color: var(--accent);
  display: inline-flex;
  flex: 0 0 auto;
  height: 46px;
  justify-content: center;
  width: 46px;
}

.recharge-hero div {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.recharge-hero strong {
  font-size: 17px;
}

.recharge-hero p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.4;
  margin: 0;
}

.recharge-form {
  display: grid;
  gap: 14px;
}

.recharge-amount {
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  gap: 3px;
  min-height: 68px;
  padding: 8px 13px;
}

.recharge-amount:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.recharge-amount.is-invalid,
.recharge-amount.is-invalid:focus-within {
  border-color: var(--negative);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--negative) 22%, transparent);
}

.recharge-amount > span:first-child {
  color: var(--muted);
  font-size: 11px;
  font-weight: 650;
}

.recharge-amount__control {
  align-items: center;
  display: flex;
  min-width: 0;
}

.recharge-amount input {
  background: transparent;
  border: 0;
  color: var(--ink);
  flex: 1;
  font-size: 25px;
  font-weight: 730;
  min-height: 36px;
  min-width: 0;
  outline: 0;
  padding: 0;
}

.recharge-amount b {
  color: var(--muted-strong);
  flex: 0 0 auto;
  font-size: 13px;
}

.quick-values {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.quick-values button {
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  color: var(--ink);
  font-size: 11px;
  font-weight: 680;
  min-height: 44px;
  min-width: 0;
  padding: 4px;
}

.quick-values button.is-active {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent);
  font-weight: 760;
}

.recharge-form dl {
  border-top: 1px solid var(--line);
  display: grid;
  margin: 2px 0 0;
}

.recharge-form dl div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(90px, auto) minmax(0, 1fr);
  min-height: 46px;
}

.recharge-form dt,
.recharge-form dd {
  font-size: 12px;
  margin: 0;
}

.recharge-form dt {
  color: var(--muted);
}

.recharge-form dd {
  color: var(--muted-strong);
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.order-result {
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: var(--radius);
  display: grid;
  gap: 11px;
  padding: 14px;
}

.order-result > div {
  align-items: center;
  color: var(--muted-strong);
  display: flex;
  font-size: 12px;
  gap: 8px;
  min-width: 0;
}

.order-result > strong {
  font-size: 18px;
  overflow-wrap: anywhere;
}

.order-result p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.45;
  margin: 0;
}

.history {
  border-top: 1px solid var(--line);
}

.history .section-heading {
  font-size: 15px;
  margin: 20px 0 0;
}

.history-list {
  display: grid;
}

.history-row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) minmax(88px, auto);
  min-height: 68px;
  padding: 9px 0;
}

.history-row > div,
.history-row > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.history-row__identity {
  align-items: center;
  display: flex;
  gap: 7px;
  min-width: 0;
}

.history-row strong,
.history-row b {
  font-size: 12px;
}

.history-row__status {
  border: 1px solid var(--line);
  border-radius: 999px;
  font-size: 10px;
  line-height: 1;
  max-width: 112px;
  overflow: hidden;
  padding: 5px 7px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-row__status.is-positive {
  background: var(--positive-soft);
  border-color: color-mix(in srgb, var(--positive) 28%, var(--line));
  color: var(--positive);
}

.history-row__status.is-negative {
  background: var(--negative-soft);
  border-color: color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
}

.history-row__status.is-pending {
  background: var(--soft);
  color: var(--muted-strong);
}

.history-row small {
  color: var(--muted);
  font-size: 11px;
}

.history-row > span {
  text-align: right;
}

@keyframes pulse {
  from { opacity: .45; }
  to { opacity: 1; }
}

@media (max-width: 340px) {
  .recharge-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .quick-values {
    gap: 6px;
  }

  .quick-values button {
    font-size: 10px;
  }

  .history-row {
    gap: 9px;
    grid-template-columns: minmax(0, 1fr) minmax(78px, auto);
  }
}
</style>
