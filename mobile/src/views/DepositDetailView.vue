<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Check, Copy, LoaderCircle } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { toDataURL } from 'qrcode'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { createDepositAddress, fetchDepositAssets } from '@/api/wallet'
import { formatAmount } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { DepositAddress } from '@/core/types'

const props = defineProps<{ asset: string; network: string }>()
const session = useSessionStore()
const { t } = useI18n()
const address = ref<DepositAddress | null>(null)
const qrUrl = ref('')
const error = ref('')
const loading = ref(false)
const copied = ref(false)

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const assets = await fetchDepositAssets()
    const minimum = assets.find((item) => item.symbol === props.asset.toUpperCase())?.minDepositAmount || 0
    address.value = await createDepositAddress(props.asset, props.network, minimum)
    qrUrl.value = await toDataURL(address.value.address, { width: 248, margin: 1, color: { dark: '#101214', light: '#ffffff' } })
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('deposit.addressFailed'))
  } finally {
    loading.value = false
  }
}

async function copyAddress(): Promise<void> {
  if (!address.value) return
  try {
    await navigator.clipboard.writeText(address.value.address)
  } catch {
    const textArea = document.createElement('textarea')
    textArea.value = address.value.address
    document.body.appendChild(textArea)
    textArea.select()
    document.execCommand('copy')
    textArea.remove()
  }
  copied.value = true
  window.setTimeout(() => { copied.value = false }, 1_800)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader
      :back="true"
      :eyebrow="t('assets.deposit')"
      :subtitle="t('deposit.networkWarning')"
      :title="t('deposit.title', { asset: asset.toUpperCase() })"
    />
    <div class="page-content deposit-detail">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('deposit.detailLoginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="deposit-detail__loading" role="status">
          <LoaderCircle :size="24" class="spin" aria-hidden="true" />
          <span>{{ t('deposit.generatingAddress') }}</span>
        </div>
        <template v-else-if="address">
          <section class="deposit-detail__primary">
            <img v-if="qrUrl" :src="qrUrl" class="deposit-detail__qr" :alt="t('deposit.qrAlt', { asset })" />
            <div class="deposit-detail__address">
              <span>{{ t('deposit.address') }}</span>
              <strong class="numeric">{{ address.address }}</strong>
              <button class="deposit-detail__copy" type="button" :aria-label="t('deposit.copyAddress')" @click="copyAddress">
                <Check v-if="copied" :size="20" aria-hidden="true" />
                <Copy v-else :size="20" aria-hidden="true" />
              </button>
            </div>
          </section>
          <dl class="deposit-detail__rows">
            <div><dt>{{ t('deposit.network') }}</dt><dd>{{ network }}</dd></div>
            <div><dt>{{ t('deposit.account') }}</dt><dd>{{ t('assets.fundingAccount') }}</dd></div>
            <div><dt>{{ t('deposit.minimum') }}</dt><dd class="numeric">{{ formatAmount(address.minDepositAmount) }} {{ address.assetSymbol }}</dd></div>
            <div><dt>{{ t('deposit.arrivalTime') }}</dt><dd>{{ t('deposit.estimatedMinutes', { minutes: network.toLowerCase().includes('eth') ? 7 : 1 }) }}</dd></div>
            <div v-if="address.memo"><dt>{{ t('deposit.memo') }}</dt><dd class="numeric">{{ address.memo }}</dd></div>
          </dl>
          <section class="deposit-detail__notice">
            <strong>{{ t('deposit.assetWarning', { asset: address.assetSymbol }) }}</strong>
            <p>{{ t('deposit.networkWarning') }}</p>
          </section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.deposit-detail {
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.deposit-detail__loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.deposit-detail__primary {
  border-bottom: 1px solid var(--line);
  padding: 2px 0 20px;
}

.deposit-detail__qr {
  border: 1px solid var(--line);
  border-radius: var(--radius);
  box-shadow: var(--shadow-soft);
  display: block;
  height: min(58vw, 232px);
  margin: 0 auto 24px;
  max-height: 232px;
  max-width: 232px;
  padding: 7px;
  width: min(58vw, 232px);
}

.deposit-detail__address {
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  gap: 5px 10px;
  grid-template-columns: minmax(0, 1fr) 48px;
  padding: 11px 10px 11px 13px;
}

.deposit-detail__address span {
  color: var(--muted);
  font-size: 11px;
  font-weight: 650;
  grid-column: 1;
}

.deposit-detail__address strong {
  font-size: 13px;
  line-height: 1.45;
  min-width: 0;
  overflow-wrap: anywhere;
}

.deposit-detail__copy {
  align-self: center;
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  color: var(--accent);
  display: grid;
  grid-column: 2;
  grid-row: 1 / span 2;
  height: 44px;
  place-items: center;
  width: 44px;
}

.deposit-detail__rows {
  display: grid;
  margin: 8px 0 0;
}

.deposit-detail__rows div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(96px, .8fr) minmax(0, 1.2fr);
  min-height: 58px;
}

.deposit-detail__rows dt {
  color: var(--ink);
  font-size: 14px;
}

.deposit-detail__rows dd {
  color: var(--muted-strong);
  font-size: 13px;
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.deposit-detail__notice {
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: var(--radius);
  color: var(--muted-strong);
  margin-top: 16px;
  padding: 14px;
}

.deposit-detail__notice strong {
  color: var(--accent);
  font-size: 13px;
}

.deposit-detail__notice p {
  font-size: 12px;
  line-height: 1.55;
  margin: 7px 0 0;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .deposit-detail__qr {
    height: min(56vw, 196px);
    width: min(56vw, 196px);
  }

  .deposit-detail__rows div {
    gap: 10px;
    grid-template-columns: minmax(88px, .75fr) minmax(0, 1.25fr);
  }
}
</style>
