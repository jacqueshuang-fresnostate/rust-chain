<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, LoaderCircle, ShieldCheck, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchDepositNetworks, fetchWalletAccounts, fetchWithdrawalAssets, submitWithdrawal, type WithdrawalAsset } from '@/api/wallet'
import { formatAmount } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
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
const reviewOpen = ref(false)
const reviewDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapReviewFocus } = useModalDialog(reviewOpen, reviewDialog, '[data-dialog-cancel]')

const available = computed(() => account.value?.available || 0)
const fee = computed(() => asset.value?.withdrawFee || 0)
const numericAmount = computed(() => Number(amount.value))
const receiveAmount = computed(() => Math.max(0, Number(amount.value || 0) - fee.value))
const addressInvalid = computed(() => validationAttempted.value && !address.value.trim())
const amountInvalid = computed(() => validationAttempted.value && (!Number.isFinite(numericAmount.value) || numericAmount.value <= 0 || numericAmount.value > available.value))
const selectedNetworkLabel = computed(() => {
  return networks.value.find((network) => network.network === selectedNetwork.value)?.displayName
    || selectedNetwork.value
    || t('withdraw.reviewedNetwork')
})

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

function requestSubmit(): void {
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
  reviewOpen.value = true
}

function closeReview(): void {
  if (submitting.value) return
  reviewOpen.value = false
  error.value = ''
}

async function submit(): Promise<void> {
  if (!asset.value || !reviewOpen.value) return
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
    reviewOpen.value = false
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('withdraw.failed'))
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
  <main
    class="page page--plain pencil-page wallet-pencil-page withdraw-pencil"
    data-pencil-source="Qa9dW o8Wsh"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.withdraw')"
      :fallback="{ name: 'withdraw-asset' }"
      :pencil="true"
      :subtitle="t('withdraw.notice')"
      :title="t('assets.withdraw')"
    />
    <div class="page-content withdraw-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('withdraw.loginDescription')"
      />
      <template v-else>
        <p v-if="error && !reviewOpen" id="withdraw-error" class="error-message wallet-feedback" role="alert">{{ error }}</p>
        <div v-if="loading" class="withdraw-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('withdraw.loading') }}</span>
        </div>
        <template v-else-if="asset">
          <section class="withdraw-identity">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="34" />
            <div class="withdraw-identity__asset">
              <strong>{{ asset.symbol }}</strong>
              <label class="withdraw-identity__network">
                <span class="sr-only">{{ t('withdraw.network') }}</span>
                <select v-model="selectedNetwork" :aria-label="t('withdraw.network')">
                  <option v-for="network in networks" :key="network.network" :value="network.network">{{ network.displayName }}</option>
                  <option v-if="!networks.length" value="">{{ t('withdraw.reviewedNetwork') }}</option>
                </select>
                <ChevronDown :size="14" aria-hidden="true" />
              </label>
            </div>
            <span class="withdraw-identity__balance">
              <small>{{ t('withdraw.availableBalance') }}</small>
              <strong class="numeric">{{ formatAmount(available) }} {{ asset.symbol }}</strong>
            </span>
          </section>
          <form class="withdraw-workflow" @submit.prevent="requestSubmit">
            <label class="withdraw-field" :class="{ 'is-invalid': addressInvalid }">
              <span class="withdraw-field__top">
                <span>{{ t('withdraw.address') }}</span>
                <small>{{ selectedNetworkLabel }}</small>
              </span>
              <div class="withdraw-field__control">
                <input
                  v-model="address"
                  autocomplete="off"
                  :aria-invalid="addressInvalid"
                  :aria-describedby="addressInvalid ? 'withdraw-error' : undefined"
                  :placeholder="t('withdraw.addressPlaceholder')"
                />
              </div>
              <small v-if="addressInvalid" class="withdraw-field__error">{{ t('withdraw.invalidRequest') }}</small>
            </label>
            <label class="withdraw-field" :class="{ 'is-invalid': amountInvalid }">
              <span class="withdraw-field__top">
                <span>{{ t('withdraw.quantity') }}</span>
                <small>{{ asset.symbol }}</small>
              </span>
              <div class="withdraw-field__control amount-shell">
                <input
                  v-model="amount"
                  inputmode="decimal"
                  :aria-invalid="amountInvalid"
                  :aria-describedby="amountInvalid ? 'withdraw-error' : undefined"
                  :placeholder="t('withdraw.minimumPlaceholder')"
                />
                <button type="button" @click="useMaximum">{{ t('withdraw.all') }}</button>
              </div>
              <small v-if="amountInvalid" class="withdraw-field__error">{{ t('withdraw.invalidRequest') }}</small>
            </label>
            <section class="withdraw-estimate">
              <div><span>{{ t('withdraw.networkFee') }}</span><strong class="numeric">{{ formatAmount(fee) }} {{ asset.symbol }}</strong></div>
              <div><span>{{ t('withdraw.estimatedArrival') }}</span><strong class="numeric up">{{ formatAmount(receiveAmount) }} {{ asset.symbol }}</strong></div>
            </section>
            <section class="security-section">
              <div class="security-section__title">
                <ShieldCheck :size="16" aria-hidden="true" />
                <span>{{ t('withdraw.security') }}</span>
              </div>
              <label class="withdraw-field">
                <span class="withdraw-field__top">
                  <span>{{ t('withdraw.fundPassword') }}</span>
                  <small>{{ t('common.optional') }}</small>
                </span>
                <div class="withdraw-field__control">
                  <input v-model="fundPassword" type="password" autocomplete="off" :placeholder="t('withdraw.fundPasswordPlaceholder')" />
                </div>
              </label>
              <label class="withdraw-field">
                <span class="withdraw-field__top">
                  <span>{{ t('withdraw.twoFactorCode') }}</span>
                  <small>{{ t('common.optional') }}</small>
                </span>
                <div class="withdraw-field__control">
                  <input v-model="totpCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('withdraw.twoFactorPlaceholder')" />
                </div>
              </label>
            </section>
            <p v-if="success" class="success-message" aria-live="polite">{{ success }}</p>
            <button class="button button--primary button--full withdraw-submit" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('withdraw.submit') }}</button>
            <p class="withdraw-notice">{{ t('withdraw.notice') }}</p>
            <nav class="withdraw-shortcuts" :aria-label="t('assets.fundTools')">
              <button type="button" @click="router.push({ name: 'withdrawal-records' })">{{ t('withdrawRecords.title') }}</button>
              <button type="button" @click="router.push({ name: 'wallet-ledger' })">{{ t('assets.ledger') }}</button>
            </nav>
          </form>
        </template>
        <p v-else-if="!loading" class="empty-state">{{ t('withdraw.unavailable') }}</p>
      </template>
    </div>

    <div v-if="reviewOpen && asset" class="withdraw-review-mask" @click.self="closeReview">
      <section
        ref="reviewDialog"
        class="withdraw-review"
        role="dialog"
        aria-modal="true"
        aria-labelledby="withdraw-review-title"
        aria-describedby="withdraw-review-description"
        @keydown="handleReviewKeydown"
      >
        <header>
          <div>
            <span>{{ t('assets.withdraw') }}</span>
            <h2 id="withdraw-review-title">{{ t('withdraw.submit') }}</h2>
            <small>{{ t('withdraw.notice') }}</small>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="submitting"
            @click="closeReview"
          >
            <X :size="21" aria-hidden="true" />
          </button>
        </header>
        <dl class="withdraw-review__summary">
          <div><dt>{{ t('withdraw.network') }}</dt><dd>{{ selectedNetwork || t('withdraw.reviewedNetwork') }}</dd></div>
          <div><dt>{{ t('withdraw.address') }}</dt><dd class="numeric">{{ address }}</dd></div>
          <div><dt>{{ t('withdraw.quantity') }}</dt><dd class="numeric">{{ formatAmount(numericAmount) }} {{ asset.symbol }}</dd></div>
          <div><dt>{{ t('withdraw.networkFee') }}</dt><dd class="numeric">{{ formatAmount(fee) }} {{ asset.symbol }}</dd></div>
          <div><dt>{{ t('withdraw.estimatedArrival') }}</dt><dd class="numeric up">{{ formatAmount(receiveAmount) }} {{ asset.symbol }}</dd></div>
        </dl>
        <p id="withdraw-review-description" class="withdraw-review__notice">{{ t('withdraw.notice') }}</p>
        <p v-if="error" class="withdraw-review__error" role="alert">{{ error }}</p>
        <div class="withdraw-review__actions">
          <button class="button button--secondary" type="button" :disabled="submitting" data-dialog-cancel @click="closeReview">
            {{ t('common.cancel') }}
          </button>
          <button class="button button--primary" type="button" :disabled="submitting" :aria-busy="submitting" @click="submit">
            {{ submitting ? t('common.submitting') : t('withdraw.submit') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.withdraw-page {
  display: grid;
  gap: 12px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.withdraw-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 160px;
}

.withdraw-identity {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  min-height: 42px;
  padding: 2px 0 6px;
}

.withdraw-identity__asset {
  align-items: center;
  display: flex;
  min-width: 0;
}

.withdraw-identity__asset > strong {
  flex: 0 0 auto;
  font-size: 15px;
  line-height: 21px;
}

.withdraw-identity__network {
  align-items: center;
  color: var(--muted);
  display: flex;
  min-height: 44px;
  min-width: 0;
  position: relative;
}

.withdraw-identity__network::before {
  content: '·';
  padding-inline: 5px;
}

.withdraw-identity__network select {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--ink);
  font: inherit;
  font-size: 13px;
  font-weight: 700;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  padding: 0 18px 0 0;
  width: 100%;
}

.withdraw-identity__network svg {
  color: var(--muted);
  pointer-events: none;
  position: absolute;
  right: 0;
}

.withdraw-identity__balance {
  display: grid;
  gap: 2px;
  justify-items: end;
  max-width: 122px;
  min-width: 0;
  text-align: right;
}

.withdraw-identity__balance small {
  color: var(--muted);
  font-size: 10px;
  font-weight: 500;
  line-height: 14px;
}

.withdraw-identity__balance strong {
  font-size: 11px;
  line-height: 15px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.withdraw-workflow {
  display: grid;
  gap: 12px;
}

.withdraw-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  gap: 5px;
  min-height: 60px;
  padding: 8px 12px;
  transition: border-color var(--motion-fast) var(--motion-ease), box-shadow var(--motion-fast) var(--motion-ease);
}

.withdraw-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--positive);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.withdraw-field.is-invalid,
.withdraw-field.is-invalid:focus-within {
  border-color: var(--negative);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--negative) 22%, transparent);
}

.withdraw-field__top {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  font-weight: 500;
  gap: 10px;
  justify-content: space-between;
  line-height: 15px;
}

.withdraw-field__top small {
  color: var(--muted);
  font-size: 10px;
  font-weight: 500;
}

.withdraw-field__control {
  align-items: center;
  display: flex;
  min-width: 0;
}

.withdraw-field input {
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
  color: var(--ink);
  font: inherit;
  font-size: 14px;
  font-weight: 600;
  min-height: 32px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.withdraw-field input:focus-visible {
  box-shadow: none;
  outline: 0;
}

.withdraw-field__error {
  color: var(--negative);
  font-size: 10px;
  font-weight: 500;
  line-height: 14px;
}

.amount-shell {
  gap: 8px;
}

.amount-shell button {
  align-items: center;
  background: transparent;
  color: var(--positive);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 700;
  justify-content: center;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
}

.withdraw-estimate {
  display: grid;
  gap: 8px;
}

.withdraw-estimate div {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 18px;
}

.withdraw-estimate span {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
}

.withdraw-estimate strong {
  font-size: 12px;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.security-section {
  border-top: 1px solid var(--hairline);
  display: grid;
  gap: 12px;
  padding-top: 12px;
}

.security-section__title {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 12px;
  font-weight: 600;
  gap: 7px;
  min-height: 18px;
}

.security-section__title svg {
  color: var(--positive);
}

.success-message {
  color: var(--positive);
  font-size: 12px;
  font-weight: 600;
  margin: 0;
}

.withdraw-submit {
  border-radius: var(--wallet-pill-radius, 999px);
  height: 48px;
  min-height: 48px;
}

.withdraw-notice {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.withdraw-shortcuts {
  border-top: 1px solid var(--hairline);
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  padding-top: 8px;
}

.withdraw-shortcuts button {
  background: transparent;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  color: var(--ink);
  font-size: 11px;
  font-weight: 600;
  min-height: 44px;
  padding: 0 8px;
}

.wallet-feedback {
  margin: 0;
}

.wallet-login-prompt {
  background: transparent;
  background-image: none;
  border: 0;
  border-top: 1px solid var(--hairline);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  min-height: 72px;
  padding: 10px 0;
}

.wallet-login-prompt :deep(.login-required__icon) {
  background: var(--accent-soft);
  border: 0;
  color: var(--positive);
  height: 34px;
  width: 34px;
}

.wallet-login-prompt :deep(.login-required__copy) {
  gap: 2px;
}

.wallet-login-prompt :deep(.login-required__copy strong) {
  font-size: 13px;
}

.wallet-login-prompt :deep(.login-required__copy p) {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.4;
}

.wallet-login-prompt :deep(.button) {
  border-radius: var(--wallet-pill-radius, 999px);
  min-height: 44px;
  padding-inline: 14px;
}

.withdraw-review-mask {
  align-items: flex-end;
  background: var(--overlay);
  display: flex;
  inset: 0;
  justify-content: center;
  padding:
    max(16px, env(safe-area-inset-top))
    16px
    max(16px, env(safe-area-inset-bottom));
  position: fixed;
  z-index: var(--layer-overlay);
}

.withdraw-review {
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: var(--wallet-sheet-radius, 20px) var(--wallet-sheet-radius, 20px) 0 0;
  display: grid;
  gap: 14px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 18px;
  width: 100%;
}

.withdraw-review > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.withdraw-review > header > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.withdraw-review > header span {
  color: var(--positive);
  font-size: 10px;
  font-weight: 700;
}

.withdraw-review h2 {
  font-size: 18px;
  margin: 0;
}

.withdraw-review > header small {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.4;
}

.withdraw-review__summary {
  border-block: 1px solid var(--hairline);
  display: grid;
  margin: 0;
}

.withdraw-review__summary > div {
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.withdraw-review__summary > div:last-child {
  border-bottom: 0;
}

.withdraw-review__summary dt,
.withdraw-review__summary dd {
  font-size: 11px;
  margin: 0;
}

.withdraw-review__summary dt {
  color: var(--muted);
  flex: 0 0 auto;
}

.withdraw-review__summary dd {
  font-weight: 700;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.withdraw-review__notice {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  padding: 9px 10px;
}

.withdraw-review__error {
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.withdraw-review__actions {
  display: grid;
  gap: 9px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.withdraw-review__actions .button {
  min-height: 48px;
  padding-inline: 10px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .withdraw-page {
    padding-inline: 16px;
  }

  .withdraw-identity {
    gap: 8px;
  }

  .withdraw-identity__balance {
    max-width: 94px;
  }

  .withdraw-review__actions {
    grid-template-columns: 1fr;
  }

  .wallet-login-prompt {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .wallet-login-prompt :deep(.button) {
    grid-column: 2;
    justify-self: start;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }

  .withdraw-field {
    transition: none;
  }
}
</style>
