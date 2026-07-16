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

const platformTarget = computed<'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web'>(() => {
  return detectClientPlatform()
})

const numericAmount = computed(() => Number(amount.value || 0))
const amountValid = computed(() => {
  if (!config.value || !Number.isFinite(numericAmount.value)) return false
  return numericAmount.value >= config.value.minAmount && (!config.value.maxAmount || numericAmount.value <= config.value.maxAmount)
})

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
  if (!config.value || !amountValid.value) {
    error.value = t('quickRecharge.invalidAmount')
    return
  }
  submitting.value = true
  try {
    const order = await createQuickRechargeOrder(numericAmount.value, platformTarget.value)
    submittedOrder.value = order
    orders.value = [order, ...orders.value.filter((item) => item.id !== order.id)]
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

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('quickRecharge.title')" />
    <div class="page-content recharge-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('quickRecharge.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('quickRecharge.loading') }}</p>
        <template v-else-if="config">
          <section class="recharge-hero surface"><span><Landmark :size="23" /></span><div><strong>{{ t('quickRecharge.hero', { token: config.token }) }}</strong><p>{{ t('quickRecharge.heroDescription') }}</p></div></section>
          <template v-if="config.enabled">
            <section class="recharge-form"><label><span>{{ t('quickRecharge.paymentAmount') }}</span><div class="recharge-amount"><input v-model="amount" inputmode="decimal" /><b>{{ config.currency }}</b></div></label><div class="quick-values"><button v-for="value in [config.minAmount, config.minAmount * 2, config.minAmount * 5]" :key="value" type="button" @click="setAmount(value)">{{ formatFiat(value, config.currency) }}</button></div><dl><div><dt>{{ t('quickRecharge.receivedAsset') }}</dt><dd>{{ config.token }}</dd></div><div><dt>{{ t('quickRecharge.network') }}</dt><dd>{{ config.network || t('quickRecharge.providerNetwork') }}</dd></div><div><dt>{{ t('quickRecharge.amountRange') }}</dt><dd>{{ formatFiat(config.minAmount, config.currency) }}<span v-if="config.maxAmount"> - {{ formatFiat(config.maxAmount, config.currency) }}</span></dd></div></dl><button class="button button--primary button--full" type="button" :disabled="submitting" @click="submit">{{ submitting ? t('quickRecharge.creating') : t('quickRecharge.buy', { token: config.token }) }}</button></section>
            <section v-if="submittedOrder" class="order-result surface"><div><ReceiptText :size="20" /><span>{{ t('quickRecharge.order', { id: submittedOrder.orderId }) }}</span></div><strong>{{ formatAmount(submittedOrder.actualAmount || submittedOrder.fiatAmount) }} {{ submittedOrder.actualAmount ? submittedOrder.token : submittedOrder.currency }}</strong><button v-if="submittedOrder.paymentUrl" class="button button--secondary button--full" type="button" @click="continuePayment">{{ t('quickRecharge.continuePayment') }} <ExternalLink :size="16" /></button><p v-else>{{ t('quickRecharge.paymentPreparing') }}</p></section>
          </template>
          <p v-else class="surface-note">{{ t('quickRecharge.disabled') }}</p>
          <section class="history"><div class="section-heading"><span>{{ t('quickRecharge.recentOrders') }}</span></div><article v-for="order in orders" :key="order.id" class="history-row"><div><strong>{{ order.token }} · {{ order.status }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div><span><b>{{ formatFiat(order.fiatAmount, order.currency) }}</b><small>{{ order.network || t('quickRecharge.quickPayment') }}</small></span></article><p v-if="!orders.length" class="empty-state">{{ t('quickRecharge.empty') }}</p></section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.recharge-page { display: grid; gap: 18px; padding-bottom: 38px; padding-top: 16px; }.recharge-hero { align-items: center; display: flex; gap: 13px; padding: 16px; }.recharge-hero > span { align-items: center; background: var(--positive-soft); border-radius: 50%; color: var(--positive); display: inline-flex; height: 46px; justify-content: center; width: 46px; }.recharge-hero div { display: grid; gap: 5px; }.recharge-hero strong { font-size: 17px; }.recharge-hero p { color: var(--muted); font-size: 13px; margin: 0; }.recharge-form { display: grid; gap: 14px; }.recharge-form label { display: grid; gap: 8px; }.recharge-form label > span { color: var(--muted); font-size: 13px; }.recharge-amount { align-items: center; background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); display: flex; min-height: 62px; padding: 0 14px; }.recharge-amount:focus-within { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }.recharge-amount input { background: transparent; border: 0; color: var(--ink); flex: 1; font-size: 26px; font-weight: 730; min-width: 0; outline: 0; }.recharge-amount b { color: var(--muted-strong); font-size: 14px; }.quick-values { display: grid; gap: 8px; grid-template-columns: repeat(3, 1fr); }.quick-values button { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--ink); font-size: 12px; min-height: 36px; }.recharge-form dl { border-top: 1px solid var(--line); display: grid; gap: 0; margin: 2px 0 0; }.recharge-form dl div { align-items: center; display: flex; justify-content: space-between; min-height: 42px; }.recharge-form dt,.recharge-form dd { font-size: 13px; margin: 0; }.recharge-form dt { color: var(--muted); }.recharge-form dd { color: var(--muted-strong); text-align: right; }.order-result { display: grid; gap: 11px; padding: 15px; }.order-result > div { align-items: center; color: var(--muted-strong); display: flex; font-size: 13px; gap: 8px; }.order-result > strong { font-size: 18px; }.order-result p { color: var(--muted); font-size: 12px; margin: 0; }.history { border-top: 1px solid var(--line); }.history .section-heading { margin-top: 20px; }.history-row { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 61px; }.history-row div,.history-row > span { display: grid; gap: 4px; }.history-row strong,.history-row b { font-size: 13px; }.history-row small { color: var(--muted); font-size: 11px; }.history-row > span { text-align: right; }
</style>
