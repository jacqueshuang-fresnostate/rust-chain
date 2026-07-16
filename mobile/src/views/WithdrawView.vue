<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, ShieldCheck } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchDepositNetworks, fetchWalletAccounts, fetchWithdrawalAssets, submitWithdrawal, type WithdrawalAsset } from '@/api/wallet'
import { formatAmount } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { DepositNetwork, WalletAccount } from '@/core/types'

const props = defineProps<{ asset: string }>()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const asset = ref<WithdrawalAsset | null>(null)
const account = ref<WalletAccount | null>(null)
const networks = ref<DepositNetwork[]>([])
const selectedNetwork = ref('')
const address = ref('')
const amount = ref('')
const fundPassword = ref('')
const totpCode = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')

const available = computed(() => account.value?.available || 0)
const fee = computed(() => asset.value?.withdrawFee || 0)
const receiveAmount = computed(() => Math.max(0, Number(amount.value || 0) - fee.value))

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [assets, accounts, networkRows] = await Promise.all([
      fetchWithdrawalAssets(),
      fetchWalletAccounts(),
      fetchDepositNetworks(props.asset),
    ])
    asset.value = assets.find((item) => item.symbol === props.asset.toUpperCase()) || null
    account.value = accounts.find((item) => item.symbol === props.asset.toUpperCase()) || null
    networks.value = networkRows
    selectedNetwork.value = networkRows[0]?.network || ''
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('withdraw.loadFailed'))
  } finally {
    loading.value = false
  }
}

function useMaximum(): void {
  amount.value = String(Math.max(0, available.value - fee.value))
}

async function submit(): Promise<void> {
  error.value = ''
  success.value = ''
  const numericAmount = Number(amount.value)
  if (!asset.value || !address.value.trim() || !Number.isFinite(numericAmount) || numericAmount <= 0) {
    error.value = t('withdraw.invalidRequest')
    return
  }
  if (numericAmount > available.value) {
    error.value = t('withdraw.exceedsBalance')
    return
  }
  submitting.value = true
  try {
    await submitWithdrawal({
      assetSymbol: asset.value.symbol,
      network: selectedNetwork.value || undefined,
      address: address.value,
      amount: numericAmount,
      fee: fee.value,
      fundPassword: fundPassword.value || undefined,
      totpCode: totpCode.value || undefined,
    })
    success.value = t('withdraw.success')
    amount.value = ''
    fundPassword.value = ''
    totpCode.value = ''
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('withdraw.failed'))
  } finally {
    submitting.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('withdraw.title', { asset: asset?.symbol || props.asset.toUpperCase() })" />
    <div class="page-content withdraw-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('withdraw.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('withdraw.loading') }}</p>
        <template v-else-if="asset">
          <section class="withdraw-balance surface">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="42" />
            <div><span>{{ t('withdraw.availableBalance') }}</span><strong class="numeric">{{ formatAmount(available) }} {{ asset.symbol }}</strong></div>
            <button type="button" @click="router.push({ name: 'wallet-ledger' })">{{ t('assets.ledger') }}</button>
          </section>
          <label class="withdraw-field"><span>{{ t('withdraw.network') }}</span><div class="select-shell"><select v-model="selectedNetwork"><option v-for="network in networks" :key="network.network" :value="network.network">{{ network.displayName }}</option><option v-if="!networks.length" value="">{{ t('withdraw.reviewedNetwork') }}</option></select><ChevronDown :size="18" /></div></label>
          <label class="withdraw-field"><span>{{ t('withdraw.address') }}</span><textarea v-model="address" rows="3" :placeholder="t('withdraw.addressPlaceholder')" /></label>
          <label class="withdraw-field"><span>{{ t('withdraw.quantity') }}</span><div class="amount-shell"><input v-model="amount" class="input" inputmode="decimal" :placeholder="t('withdraw.minimumPlaceholder')" /><b>{{ asset.symbol }}</b><button type="button" @click="useMaximum">{{ t('withdraw.all') }}</button></div></label>
          <section class="withdraw-estimate"><div><span>{{ t('withdraw.networkFee') }}</span><strong>{{ formatAmount(fee) }} {{ asset.symbol }}</strong></div><div><span>{{ t('withdraw.estimatedArrival') }}</span><strong class="up">{{ formatAmount(receiveAmount) }} {{ asset.symbol }}</strong></div></section>
          <section class="security-section"><div class="security-section__title"><ShieldCheck :size="19" /><span>{{ t('withdraw.security') }}</span></div><label><span>{{ t('withdraw.fundPassword') }}</span><input v-model="fundPassword" class="input" type="password" autocomplete="off" :placeholder="t('withdraw.fundPasswordPlaceholder')" /></label><label><span>{{ t('withdraw.twoFactorCode') }}</span><input v-model="totpCode" class="input" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('withdraw.twoFactorPlaceholder')" /></label></section>
          <p v-if="success" class="success-message">{{ success }}</p>
          <button class="button button--primary button--full" type="button" :disabled="submitting" @click="submit">{{ submitting ? t('common.submitting') : t('withdraw.submit') }}</button>
          <p class="withdraw-notice">{{ t('withdraw.notice') }}</p>
        </template>
        <p v-else-if="!loading" class="empty-state">{{ t('withdraw.unavailable') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.withdraw-page { display: grid; gap: 18px; padding-bottom: 36px; padding-top: 16px; }.withdraw-balance { align-items: center; display: grid; gap: 12px; grid-template-columns: 42px 1fr auto; padding: 14px; }.withdraw-balance div { display: grid; gap: 4px; }.withdraw-balance span,.withdraw-field > span,.security-section label > span { color: var(--muted); font-size: 13px; }.withdraw-balance strong { font-size: 17px; }.withdraw-balance button { background: var(--soft); border-radius: 18px; color: var(--ink); font-size: 12px; font-weight: 700; min-height: 32px; padding: 0 12px; }.withdraw-field { display: grid; gap: 8px; }.withdraw-field textarea { background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); color: var(--ink); font: inherit; outline: 0; padding: 13px; resize: vertical; width: 100%; }.withdraw-field textarea:focus { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }.select-shell { align-items: center; background: var(--soft); border-radius: var(--radius); display: flex; min-height: 50px; padding: 0 13px; }.select-shell select { appearance: none; background: transparent; border: 0; color: var(--ink); flex: 1; font: inherit; outline: 0; width: 100%; }.select-shell svg { color: var(--muted); pointer-events: none; }.amount-shell { align-items: center; display: grid; grid-template-columns: 1fr auto auto; position: relative; }.amount-shell .input { padding-right: 104px; }.amount-shell b { font-size: 13px; margin-right: 8px; }.amount-shell button { background: transparent; color: var(--accent); font-size: 13px; font-weight: 720; padding: 8px 13px; }.withdraw-estimate { background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr 1fr; padding: 13px; }.withdraw-estimate div { display: grid; gap: 5px; }.withdraw-estimate div + div { border-left: 1px solid var(--line); padding-left: 13px; }.withdraw-estimate span { color: var(--muted); font-size: 12px; }.withdraw-estimate strong { font-size: 14px; }.security-section { border-top: 1px solid var(--line); display: grid; gap: 13px; padding-top: 18px; }.security-section__title { align-items: center; display: flex; font-size: 16px; font-weight: 720; gap: 8px; }.security-section__title svg { color: var(--accent); }.security-section label { display: grid; gap: 8px; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }.withdraw-notice { color: var(--muted); font-size: 12px; line-height: 1.55; margin: -4px 0 0; text-align: center; }
</style>
