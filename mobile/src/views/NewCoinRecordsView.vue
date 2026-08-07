<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  CheckCircle2,
  CircleAlert,
  LoaderCircle,
  LockKeyhole,
  PackageOpen,
  ReceiptText,
  RefreshCw,
  UnlockKeyhole,
  WalletCards,
  X,
} from 'lucide-vue-next'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  fetchNewCoinDistributions,
  fetchNewCoinProjects,
  fetchNewCoinPurchases,
  fetchNewCoinSubscriptions,
  fetchNewCoinUnlocks,
  payNewCoinUnlockFee,
  releaseNewCoinUnlock,
  type NewCoinDistribution,
  type NewCoinPurchase,
  type NewCoinSubscription,
  type NewCoinUnlock,
} from '@/api/newCoin'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import type { WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

type RecordTab = 'subscriptions' | 'distributions' | 'purchases' | 'unlocks'

const session = useSessionStore()
const { t } = useI18n()
const activeTab = ref<RecordTab>('subscriptions')
const subscriptions = ref<NewCoinSubscription[]>([])
const distributions = ref<NewCoinDistribution[]>([])
const purchases = ref<NewCoinPurchase[]>([])
const unlocks = ref<NewCoinUnlock[]>([])
const accounts = ref<WalletAccount[]>([])
const projectSymbols = ref<Record<number, string>>({})
const pendingUnlock = ref<NewCoinUnlock | null>(null)
const paymentAssetId = ref(0)
const loading = ref(false)
const saving = ref('')
const error = ref('')
const success = ref('')
const feeDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const paymentAccount = computed(() => accounts.value.find((account) => account.assetId === paymentAssetId.value))
const paymentOptions = computed(() => pendingUnlock.value?.unlockFeeAssetId ? accounts.value.filter((account) => account.assetId === pendingUnlock.value?.unlockFeeAssetId) : accounts.value)
const paymentAmount = computed(() => pendingUnlock.value?.unlockFeeAmount || 0)
const dialogOpen = computed(() => Boolean(pendingUnlock.value))
const tabs = computed<Array<{ key: RecordTab; label: string }>>(() => [
  { key: 'subscriptions', label: t('newCoin.tabSubscriptions') },
  { key: 'distributions', label: t('newCoin.tabDistributions') },
  { key: 'purchases', label: t('newCoin.tabPurchases') },
  { key: 'unlocks', label: t('newCoin.tabUnlocks') },
])

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProjects, nextSubscriptions, nextDistributions, nextPurchases, nextUnlocks, nextAccounts] = await Promise.all([
      fetchNewCoinProjects(),
      fetchNewCoinSubscriptions(),
      fetchNewCoinDistributions(),
      fetchNewCoinPurchases(),
      fetchNewCoinUnlocks(),
      fetchWalletAccounts(),
    ])
    projectSymbols.value = Object.fromEntries(nextProjects.map((project) => [project.id, project.symbol]))
    subscriptions.value = nextSubscriptions
    distributions.value = nextDistributions
    purchases.value = nextPurchases
    unlocks.value = nextUnlocks
    accounts.value = nextAccounts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.recordLoadFailed'))
  } finally {
    loading.value = false
  }
}

function projectLabel(projectId: number): string {
  return projectSymbols.value[projectId] || t('newCoin.projectNumber', { id: projectId })
}

function assetLabel(assetId: number): string {
  return accounts.value.find((account) => account.assetId === assetId)?.symbol || t('newCoin.assetNumber', { id: assetId })
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'newCoin.statusPending',
    processing: 'newCoin.statusProcessing',
    completed: 'newCoin.statusCompleted',
    allocated: 'newCoin.statusAllocated',
    distributed: 'newCoin.statusDistributed',
    locked: 'newCoin.statusLocked',
    paid: 'newCoin.statusPaid',
    unpaid: 'newCoin.statusUnpaid',
    released: 'newCoin.statusReleased',
    cancelled: 'newCoin.statusCancelled',
    canceled: 'newCoin.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function openFeePayment(unlock: NewCoinUnlock): void {
  pendingUnlock.value = unlock
  paymentAssetId.value = unlock.unlockFeeAssetId || accounts.value[0]?.assetId || 0
  error.value = ''
}

function closeFeePayment(): void {
  if (saving.value.startsWith('fee-')) return
  pendingUnlock.value = null
  error.value = ''
}

async function payFee(): Promise<void> {
  if (!pendingUnlock.value || !paymentAssetId.value || paymentAmount.value <= 0) {
    error.value = t('newCoin.invalidFeeConfig')
    return
  }
  if ((paymentAccount.value?.available || 0) < paymentAmount.value) {
    error.value = t('newCoin.insufficientFeeBalance')
    return
  }
  saving.value = `fee-${pendingUnlock.value.id}`
  error.value = ''
  try {
    await payNewCoinUnlockFee({
      idempotencyKey: pendingUnlock.value.idempotencyKey,
      paymentAssetId: paymentAssetId.value,
      amount: paymentAmount.value,
    })
    pendingUnlock.value = null
    success.value = t('newCoin.feePaid')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.feePaymentFailed'))
  } finally {
    saving.value = ''
  }
}

async function release(unlock: NewCoinUnlock): Promise<void> {
  if (!unlock.idempotencyKey) return
  saving.value = `release-${unlock.id}`
  error.value = ''
  try {
    await releaseNewCoinUnlock(unlock.idempotencyKey)
    success.value = t('newCoin.assetReleased')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.releaseUnavailable'))
  } finally {
    saving.value = ''
  }
}

function feePaid(unlock: NewCoinUnlock): boolean {
  return unlock.feePaidStatus.toLowerCase() === 'paid'
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeFeePayment()
    return
  }
  if (event.key !== 'Tab' || !feeDialog.value) return
  const focusable = Array.from(feeDialog.value.querySelectorAll<HTMLElement>(
    'button:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
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

watch(dialogOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    feeDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
    return
  }
  document.body.style.overflow = previousBodyOverflow
  await nextTick()
  returnFocus?.focus()
  returnFocus = null
})

onMounted(() => { void load() })

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main
    class="page page--plain pencil-page new-coin-records-page"
    data-pencil-source="A9It6g h4gfd"
  >
    <PageHeader
      :back="true"
      :pencil="true"
      :title="t('newCoin.recordTitle')"
    />
    <div class="page-content new-coin-records-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.recordLoginDescription')" />
      <template v-else>
        <div v-if="error && !dialogOpen" class="records-message records-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('common.retry')" @click="load">
            <RefreshCw :size="17" />
          </button>
        </div>
        <div v-if="success" class="records-message records-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ success }}</span>
        </div>
        <nav class="record-tabs" :aria-label="t('newCoin.recordCategory')">
          <button
            v-for="tab in tabs"
            :key="tab.key"
            type="button"
            :aria-pressed="activeTab === tab.key"
            :class="{ active: activeTab === tab.key }"
            @click="activeTab = tab.key"
          >
            {{ tab.label }}
          </button>
        </nav>
        <div v-if="loading" class="records-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('newCoin.loadingRecords') }}</span>
        </div>
        <template v-else>
          <section v-if="activeTab === 'subscriptions'" class="record-list">
            <article v-for="record in subscriptions" :key="record.id">
              <div class="record-icon record-icon--positive"><ReceiptText :size="19" /></div>
              <div class="record-main"><strong>{{ t('newCoin.subscriptionRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div>
              <div class="record-value"><b class="numeric">{{ formatAmount(record.requestedQuantity) }}</b><small>{{ t('newCoin.distributed', { amount: formatAmount(record.allocatedQuantity) }) }}</small><em>{{ statusLabel(record.status) }}</em></div>
            </article>
            <div v-if="!subscriptions.length" class="records-state records-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('newCoin.noSubscriptions') }}</span>
            </div>
          </section>

          <section v-else-if="activeTab === 'distributions'" class="record-list">
            <article v-for="record in distributions" :key="record.id">
              <div class="record-icon record-icon--focus"><WalletCards :size="19" /></div>
              <div class="record-main"><strong>{{ t('newCoin.distributionRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div>
              <div class="record-value"><b class="numeric">{{ formatAmount(record.quantity) }} {{ assetLabel(record.assetId) }}</b><small>{{ t(record.lockPositionId ? 'newCoin.locked' : 'newCoin.credited') }}</small><em>{{ statusLabel(record.status) }}</em></div>
            </article>
            <div v-if="!distributions.length" class="records-state records-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('newCoin.noDistributions') }}</span>
            </div>
          </section>

          <section v-else-if="activeTab === 'purchases'" class="record-list">
            <article v-for="record in purchases" :key="record.id">
              <div class="record-icon record-icon--accent"><LockKeyhole :size="19" /></div>
              <div class="record-main"><strong>{{ t('newCoin.purchaseRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div>
              <div class="record-value"><b class="numeric">{{ formatAmount(record.quantity) }} {{ assetLabel(record.baseAssetId) }}</b><small>{{ t('newCoin.paidAmount', { price: formatPrice(record.price), amount: formatAmount(record.quoteAmount) }) }}</small><em>{{ statusLabel(record.status) }}</em></div>
            </article>
            <div v-if="!purchases.length" class="records-state records-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('newCoin.noPurchases') }}</span>
            </div>
          </section>

          <section v-else class="record-list unlock-list">
            <article v-for="unlock in unlocks" :key="unlock.id">
              <div class="record-icon record-icon--accent"><UnlockKeyhole :size="19" /></div>
              <div class="record-main"><strong>{{ t('newCoin.pendingUnlock', { asset: assetLabel(unlock.assetId) }) }}</strong><small>{{ formatDateTime(unlock.createdAt) }} · {{ statusLabel(unlock.status) }}</small></div>
              <div class="record-value">
                <b class="numeric">{{ formatAmount(unlock.unlockQuantity) }} {{ assetLabel(unlock.assetId) }}</b>
                <small v-if="unlock.unlockFeeEnabled">{{ t('newCoin.feeAmount', { amount: formatAmount(unlock.unlockFeeAmount), asset: assetLabel(unlock.unlockFeeAssetId || 0) }) }}</small>
                <small v-else>{{ t('newCoin.noUnlockFee') }}</small>
                <em :class="{ paid: feePaid(unlock) }">{{ unlock.unlockFeeEnabled ? t('newCoin.feeStatus', { status: statusLabel(unlock.feePaidStatus) }) : t('newCoin.directlyReleasable') }}</em>
              </div>
              <div class="unlock-actions">
                <button
                  v-if="unlock.unlockFeeEnabled && !feePaid(unlock)"
                  class="button button--secondary"
                  type="button"
                  :disabled="saving === `fee-${unlock.id}`"
                  @click="openFeePayment(unlock)"
                >
                  {{ t('newCoin.payFee') }}
                </button>
                <button
                  v-else
                  class="button button--primary"
                  type="button"
                  :disabled="saving === `release-${unlock.id}`"
                  @click="release(unlock)"
                >
                  {{ t(saving === `release-${unlock.id}` ? 'newCoin.releasing' : 'newCoin.release') }}
                </button>
              </div>
            </article>
            <div v-if="!unlocks.length" class="records-state records-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('newCoin.noUnlocks') }}</span>
            </div>
          </section>
        </template>
      </template>
    </div>

    <div v-if="pendingUnlock" class="fee-mask" @click.self="closeFeePayment">
      <form
        ref="feeDialog"
        class="fee-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="new-coin-fee-title"
        @keydown="trapDialogFocus"
        @submit.prevent="payFee"
      >
        <header>
          <div>
            <span>{{ t('newCoin.payFee') }}</span>
            <h2 id="new-coin-fee-title">{{ t('newCoin.releaseAsset', { asset: assetLabel(pendingUnlock.assetId) }) }}</h2>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="saving.startsWith('fee-')"
            data-dialog-cancel
            @click="closeFeePayment"
          >
            <X :size="21" />
          </button>
        </header>
        <p>{{ t('newCoin.feeDescription') }}</p>
        <label class="fee-field">
          <span>{{ t('newCoin.paymentAsset') }}</span>
          <select v-model="paymentAssetId">
            <option v-for="account in paymentOptions" :key="account.assetId" :value="account.assetId">
              {{ t('newCoin.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}
            </option>
          </select>
        </label>
        <dl class="fee-summary">
          <div><dt>{{ t('newCoin.unlockFee') }}</dt><dd>{{ formatAmount(paymentAmount) }} {{ paymentAccount?.symbol || assetLabel(pendingUnlock.unlockFeeAssetId || 0) }}</dd></div>
          <div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatAmount(paymentAccount?.available) }} {{ paymentAccount?.symbol }}</dd></div>
        </dl>
        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <button
          class="button button--primary button--full fee-submit"
          type="submit"
          :disabled="saving.startsWith('fee-')"
          :aria-busy="saving.startsWith('fee-')"
        >
          {{ t(saving.startsWith('fee-') ? 'newCoin.paying' : 'newCoin.confirmPayment') }}
        </button>
      </form>
    </div>
  </main>
</template>

<style scoped>
.new-coin-records-page {
  background: var(--surface);
  min-width: 0;
}

.new-coin-records-content {
  display: grid;
  gap: 0;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 0;
}

.records-message {
  align-items: center;
  border: 1px solid currentColor;
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 1.45;
  min-height: 52px;
  padding: 4px 5px 4px 11px;
  margin-top: 12px;
}

.records-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.records-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.records-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.records-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 148px;
  text-align: center;
}

.records-state--empty {
  min-height: 112px;
}

.record-tabs {
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 44px;
  min-height: 44px;
}

.record-tabs button {
  background: var(--field-surface);
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-size: 11px;
  font-weight: 700;
  line-height: 1.15;
  height: 44px;
  min-height: 44px;
  min-width: 0;
  overflow-wrap: anywhere;
  padding: 4px 3px 1px;
}

.record-tabs button.active {
  background: var(--surface);
  border-bottom-color: var(--accent);
  color: var(--ink);
}

.record-list {
  border-top: 1px solid var(--line);
  display: grid;
  min-width: 0;
}

.record-list article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: 36px minmax(0, 1fr) minmax(88px, auto);
  min-height: 72px;
  min-width: 0;
  padding: 8px 0;
}

.record-icon {
  display: grid;
  height: 36px;
  place-items: center;
  width: 36px;
  border: 1px solid var(--line);
  border-radius: 50%;
}

.record-icon--positive {
  background: var(--positive-soft);
  color: var(--positive);
}

.record-icon--focus {
  background: color-mix(in srgb, var(--focus) 12%, var(--surface));
  color: var(--focus);
}

.record-icon--accent {
  background: var(--accent-soft);
  color: var(--accent);
}

.record-main,
.record-value {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.record-main strong,
.record-value b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.record-main small,
.record-value small {
  color: var(--muted);
  font-size: 9px;
  overflow-wrap: anywhere;
}

.record-value {
  text-align: right;
}

.record-value em {
  color: var(--accent);
  font-size: 10px;
  font-style: normal;
}

.record-value em.paid {
  color: var(--positive);
}

.unlock-list article {
  grid-template-columns: 36px minmax(0, 1fr) minmax(88px, auto);
}

.unlock-actions {
  grid-column: 2 / -1;
  justify-self: end;
}

.unlock-actions .button {
  border-radius: 0;
  font-size: 11px;
  min-height: 44px;
  min-width: 104px;
  padding: 0 11px;
}

.fee-mask {
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

.fee-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 14px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.fee-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.fee-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.fee-dialog > header span {
  color: var(--accent);
  font-size: 10px;
  font-weight: 800;
}

.fee-dialog h2 {
  font-size: 18px;
  margin: 0;
  overflow-wrap: anywhere;
}

.fee-dialog > p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.fee-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.fee-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.fee-field > span {
  color: var(--muted);
  font-size: 10px;
}

.fee-field select {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 12px;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.fee-summary {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.fee-summary > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.fee-summary > div:last-child {
  border-bottom: 0;
}

.fee-summary dt,
.fee-summary dd {
  font-size: 11px;
  margin: 0;
}

.fee-summary dt {
  color: var(--muted);
}

.fee-summary dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
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

.fee-submit {
  border-radius: 0;
  min-height: 52px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .new-coin-records-content {
    padding-left: 16px;
    padding-right: 16px;
  }
}

@media (max-width: 340px) {
  .record-tabs button {
    font-size: 9px;
  }

  .record-list article,
  .unlock-list article {
    align-items: start;
    grid-template-columns: 36px minmax(0, 1fr);
  }

  .record-icon {
    height: 36px;
    width: 36px;
  }

  .record-value {
    grid-column: 2;
    justify-items: start;
    text-align: left;
  }

  .unlock-actions {
    grid-column: 2;
    justify-self: stretch;
  }

  .unlock-actions .button {
    width: 100%;
  }

  .fee-summary > div {
    align-items: flex-start;
    flex-direction: column;
    gap: 5px;
    justify-content: center;
    padding: 8px 0;
  }

  .fee-summary dd {
    text-align: left;
  }
}
</style>
