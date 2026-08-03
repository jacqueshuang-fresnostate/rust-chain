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
  <main
    class="page page--plain pencil-page wallet-pencil-page deposit-detail-pencil"
    data-pencil-source="w5htG TCN5A"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.deposit')"
      :fallback="{ name: 'deposit-network', params: { asset } }"
      :pencil="true"
      :subtitle="t('deposit.networkWarning')"
      :title="t('deposit.address')"
    />
    <div class="page-content deposit-detail">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('deposit.detailLoginDescription')"
      />
      <template v-else>
        <p v-if="error" class="error-message wallet-feedback" role="alert">{{ error }}</p>
        <div v-if="loading" class="deposit-detail__loading" role="status">
          <LoaderCircle :size="24" class="spin" aria-hidden="true" />
          <span>{{ t('deposit.generatingAddress') }}</span>
        </div>
        <template v-else-if="address">
          <section class="deposit-detail__identity">
            <strong>{{ address.assetSymbol }} · {{ address.network }}</strong>
          </section>
          <div class="deposit-detail__qr-wrap">
            <img v-if="qrUrl" :src="qrUrl" class="deposit-detail__qr" :alt="t('deposit.qrAlt', { asset })" />
          </div>
          <p class="deposit-detail__qr-label">{{ t('deposit.qrAlt', { asset: address.assetSymbol }) }}</p>
          <section class="deposit-detail__address">
            <span>{{ t('deposit.address') }}</span>
            <strong class="numeric">{{ address.address }}</strong>
          </section>
          <button
            class="deposit-detail__copy"
            type="button"
            :aria-label="t('deposit.copyAddress')"
            @click="copyAddress"
          >
            <Check v-if="copied" :size="18" aria-hidden="true" />
            <Copy v-else :size="18" aria-hidden="true" />
            <span>{{ copied ? t('common.copied') : t('deposit.copyAddress') }}</span>
          </button>
          <dl class="deposit-detail__meta">
            <div>
              <dt>{{ t('deposit.minimum') }}</dt>
              <dd class="numeric">{{ formatAmount(address.minDepositAmount) }} {{ address.assetSymbol }}</dd>
            </div>
            <div v-if="address.memo">
              <dt>{{ t('deposit.memo') }}</dt>
              <dd class="numeric">{{ address.memo }}</dd>
            </div>
          </dl>
          <p class="deposit-detail__notice">
            {{ t('deposit.assetWarning', { asset: address.assetSymbol }) }}
            {{ t('deposit.networkWarning') }}
          </p>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.deposit-detail {
  display: grid;
  gap: 14px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
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

.deposit-detail__identity {
  align-items: center;
  display: flex;
  min-height: 22px;
}

.deposit-detail__identity strong {
  font-size: 16px;
  line-height: 22px;
}

.deposit-detail__qr-wrap {
  display: flex;
  justify-content: center;
  width: 100%;
}

.deposit-detail__qr {
  border: 1px solid var(--line);
  border-radius: var(--wallet-qr-radius, 16px);
  display: block;
  height: 180px;
  image-rendering: crisp-edges;
  padding: 26px;
  width: 180px;
}

.deposit-detail__qr-label {
  color: var(--muted);
  font-size: 11px;
  font-weight: 500;
  line-height: 15px;
  margin: 0;
}

.deposit-detail__address {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.deposit-detail__address span {
  color: var(--muted);
  font-size: 11px;
  font-weight: 500;
}

.deposit-detail__address strong {
  font-size: 13px;
  line-height: 1.45;
  min-width: 0;
  overflow-wrap: anywhere;
}

.deposit-detail__copy {
  align-items: center;
  background: var(--accent);
  border: 0;
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--on-accent);
  display: flex;
  font-size: 15px;
  font-weight: 700;
  gap: 8px;
  height: 48px;
  justify-content: center;
  min-height: 48px;
  width: 100%;
}

.deposit-detail__meta {
  display: grid;
  gap: 10px;
  margin: 0;
}

.deposit-detail__meta div {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 18px;
}

.deposit-detail__meta dt {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
}

.deposit-detail__meta dd {
  color: var(--ink);
  font-size: 11px;
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.deposit-detail__notice {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
  margin: 0;
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

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .deposit-detail {
    padding-inline: 16px;
  }

  .deposit-detail__qr {
    height: 164px;
    padding: 23px;
    width: 164px;
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
}
</style>
