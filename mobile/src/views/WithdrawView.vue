<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, LoaderCircle, ShieldCheck } from 'lucide-vue-next'
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
const validationAttempted = ref(false)

const available = computed(() => account.value?.available || 0)
const fee = computed(() => asset.value?.withdrawFee || 0)
const numericAmount = computed(() => Number(amount.value))
const receiveAmount = computed(() => Math.max(0, Number(amount.value || 0) - fee.value))
const addressInvalid = computed(() => validationAttempted.value && !address.value.trim())
const amountInvalid = computed(() => validationAttempted.value && (!Number.isFinite(numericAmount.value) || numericAmount.value <= 0 || numericAmount.value > available.value))

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
  validationAttempted.value = true
  if (!asset.value || !address.value.trim() || !Number.isFinite(numericAmount.value) || numericAmount.value <= 0) {
    error.value = t('withdraw.invalidRequest')
    return
  }
  if (numericAmount.value > available.value) {
    error.value = t('withdraw.exceedsBalance')
    return
  }
  submitting.value = true
  try {
    await submitWithdrawal({
      assetSymbol: asset.value.symbol,
      network: selectedNetwork.value || undefined,
      address: address.value,
      amount: numericAmount.value,
      fee: fee.value,
      fundPassword: fundPassword.value || undefined,
      totpCode: totpCode.value || undefined,
    })
    success.value = t('withdraw.success')
    amount.value = ''
    fundPassword.value = ''
    totpCode.value = ''
    validationAttempted.value = false
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
        <p v-if="error" id="withdraw-error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="withdraw-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('withdraw.loading') }}</span>
        </div>
        <template v-else-if="asset">
          <section class="withdraw-balance">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="42" />
            <div>
              <span>{{ t('withdraw.availableBalance') }}</span>
              <strong class="numeric">{{ formatAmount(available) }} {{ asset.symbol }}</strong>
            </div>
            <span class="withdraw-balance__links">
              <button type="button" @click="router.push({ name: 'withdrawal-records' })">{{ t('withdrawRecords.title') }}</button>
              <button type="button" @click="router.push({ name: 'wallet-ledger' })">{{ t('assets.ledger') }}</button>
            </span>
          </section>
          <form class="withdraw-workflow" @submit.prevent="submit">
            <label class="withdraw-field">
              <span>{{ t('withdraw.network') }}</span>
              <div class="select-shell">
                <select v-model="selectedNetwork">
                  <option v-for="network in networks" :key="network.network" :value="network.network">{{ network.displayName }}</option>
                  <option v-if="!networks.length" value="">{{ t('withdraw.reviewedNetwork') }}</option>
                </select>
                <ChevronDown :size="18" aria-hidden="true" />
              </div>
            </label>
            <label class="withdraw-field" :class="{ 'is-invalid': addressInvalid }">
              <span>{{ t('withdraw.address') }}</span>
              <textarea
                v-model="address"
                rows="3"
                autocomplete="off"
                :aria-invalid="addressInvalid"
                :aria-describedby="addressInvalid ? 'withdraw-error' : undefined"
                :placeholder="t('withdraw.addressPlaceholder')"
              />
            </label>
            <label class="withdraw-field" :class="{ 'is-invalid': amountInvalid }">
              <span>{{ t('withdraw.quantity') }}</span>
              <div class="amount-shell">
                <input
                  v-model="amount"
                  inputmode="decimal"
                  :aria-invalid="amountInvalid"
                  :aria-describedby="amountInvalid ? 'withdraw-error' : undefined"
                  :placeholder="t('withdraw.minimumPlaceholder')"
                />
                <b>{{ asset.symbol }}</b>
                <button type="button" @click="useMaximum">{{ t('withdraw.all') }}</button>
              </div>
            </label>
            <section class="withdraw-estimate">
              <div><span>{{ t('withdraw.networkFee') }}</span><strong class="numeric">{{ formatAmount(fee) }} {{ asset.symbol }}</strong></div>
              <div><span>{{ t('withdraw.estimatedArrival') }}</span><strong class="numeric up">{{ formatAmount(receiveAmount) }} {{ asset.symbol }}</strong></div>
            </section>
            <section class="security-section">
              <div class="security-section__title">
                <ShieldCheck :size="19" aria-hidden="true" />
                <span>{{ t('withdraw.security') }}</span>
              </div>
              <label class="withdraw-field">
                <span>{{ t('withdraw.fundPassword') }}</span>
                <input v-model="fundPassword" type="password" autocomplete="off" :placeholder="t('withdraw.fundPasswordPlaceholder')" />
              </label>
              <label class="withdraw-field">
                <span>{{ t('withdraw.twoFactorCode') }}</span>
                <input v-model="totpCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('withdraw.twoFactorPlaceholder')" />
              </label>
            </section>
            <p v-if="success" class="success-message" aria-live="polite">{{ success }}</p>
            <button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('withdraw.submit') }}</button>
            <p class="withdraw-notice">{{ t('withdraw.notice') }}</p>
          </form>
        </template>
        <p v-else-if="!loading" class="empty-state">{{ t('withdraw.unavailable') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.withdraw-page {
  display: grid;
  gap: 16px;
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.withdraw-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.withdraw-balance {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 12px;
  grid-template-columns: 42px minmax(0, 1fr);
  padding: 2px 0 16px;
}

.withdraw-balance > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.withdraw-balance > div span,
.withdraw-field > span {
  color: var(--muted);
  font-size: 11px;
  font-weight: 650;
}

.withdraw-balance strong {
  font-size: 18px;
  overflow-wrap: anywhere;
}

.withdraw-balance__links {
  display: grid;
  gap: 8px;
  grid-column: 1 / -1;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.withdraw-balance button {
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  color: var(--ink);
  font-size: 12px;
  font-weight: 700;
  min-height: 44px;
  padding: 0 10px;
}

.withdraw-workflow {
  display: grid;
  gap: 16px;
}

.withdraw-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  gap: 3px;
  padding: 8px 12px;
}

.withdraw-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.withdraw-field.is-invalid,
.withdraw-field.is-invalid:focus-within {
  border-color: var(--negative);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--negative) 22%, transparent);
}

.withdraw-field input,
.withdraw-field textarea,
.select-shell select {
  background: transparent;
  border: 0;
  color: var(--ink);
  font: inherit;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.withdraw-field input,
.select-shell select {
  min-height: 36px;
}

.withdraw-field textarea {
  line-height: 1.45;
  min-height: 76px;
  padding-top: 5px;
  resize: vertical;
}

.select-shell {
  align-items: center;
  display: flex;
  min-width: 0;
}

.select-shell select {
  appearance: none;
  flex: 1;
}

.select-shell svg {
  color: var(--muted);
  flex: 0 0 auto;
  pointer-events: none;
}

.amount-shell {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  min-width: 0;
}

.amount-shell b {
  font-size: 12px;
  margin-left: 8px;
}

.amount-shell button {
  background: transparent;
  color: var(--accent);
  font-size: 12px;
  font-weight: 750;
  min-height: 44px;
  padding: 0 4px 0 12px;
}

.withdraw-estimate {
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.withdraw-estimate div {
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 13px;
}

.withdraw-estimate div + div {
  border-left: 1px solid var(--line);
}

.withdraw-estimate span {
  color: var(--muted);
  font-size: 11px;
}

.withdraw-estimate strong {
  font-size: 13px;
  overflow-wrap: anywhere;
}

.security-section {
  border-top: 1px solid var(--line);
  display: grid;
  gap: 13px;
  padding-top: 18px;
}

.security-section__title {
  align-items: center;
  display: flex;
  font-size: 15px;
  font-weight: 720;
  gap: 8px;
}

.security-section__title svg {
  color: var(--accent);
}

.success-message {
  color: var(--positive);
  font-size: 13px;
  font-weight: 650;
  margin: 0;
}

.withdraw-notice {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
  margin: -3px 0 0;
  text-align: center;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .withdraw-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .withdraw-estimate {
    grid-template-columns: 1fr;
  }

  .withdraw-estimate div + div {
    border-left: 0;
    border-top: 1px solid var(--line);
  }
}
</style>
