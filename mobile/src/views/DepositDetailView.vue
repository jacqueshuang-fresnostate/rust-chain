<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Check, Copy, MoreHorizontal } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { toDataURL } from 'qrcode'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { createDepositAddress, fetchDepositAssets } from '@/api/wallet'
import { formatAmount, shortAddress } from '@/core/format'
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
    <PageHeader :title="t('deposit.title', { asset: asset.toUpperCase() })"><template #actions><button class="icon-button" type="button" :aria-label="t('deposit.more')"><MoreHorizontal :size="24" /></button></template></PageHeader>
    <div class="page-content deposit-detail">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('deposit.detailLoginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('deposit.generatingAddress') }}</p>
        <template v-else-if="address">
          <img :src="qrUrl" class="deposit-detail__qr" :alt="t('deposit.qrAlt', { asset })" />
          <section class="deposit-detail__address"><span>{{ t('deposit.address') }}</span><strong>{{ shortAddress(address.address, 13, 10) }}</strong><button class="icon-button" type="button" :aria-label="t('deposit.copyAddress')" @click="copyAddress"><Check v-if="copied" :size="22" /><Copy v-else :size="22" /></button></section>
          <dl class="deposit-detail__rows"><div><dt>{{ t('deposit.network') }}</dt><dd>{{ network }}</dd></div><div><dt>{{ t('deposit.account') }}</dt><dd>{{ t('assets.fundingAccount') }}</dd></div><div><dt>{{ t('deposit.minimum') }}</dt><dd>{{ formatAmount(address.minDepositAmount) }} {{ address.assetSymbol }}</dd></div><div><dt>{{ t('deposit.arrivalTime') }}</dt><dd>{{ t('deposit.estimatedMinutes', { minutes: network.toLowerCase().includes('eth') ? 7 : 1 }) }}</dd></div><div v-if="address.memo"><dt>Memo</dt><dd>{{ address.memo }}</dd></div></dl>
          <section class="deposit-detail__notice"><strong>{{ t('deposit.assetWarning', { asset: address.assetSymbol }) }}</strong><p>{{ t('deposit.networkWarning') }}</p></section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.deposit-detail { padding-top: 16px; }.deposit-detail__qr { border: 1px solid var(--line); border-radius: var(--radius); display: block; height: min(62vw, 248px); margin: 4px auto 28px; max-height: 248px; max-width: 248px; padding: 8px; width: min(62vw, 248px); }.deposit-detail__address { border-bottom: 1px solid var(--line); display: grid; gap: 7px; grid-template-columns: 1fr 44px; padding: 12px 0 22px; }.deposit-detail__address span { color: var(--muted); font-size: 13px; grid-column: 1; }.deposit-detail__address strong { font-size: 18px; line-height: 1.35; overflow-wrap: anywhere; }.deposit-detail__address button { grid-column: 2; grid-row: 1 / span 2; justify-self: end; margin-top: 8px; }
.deposit-detail__rows { display: grid; gap: 0; margin: 8px 0 0; }.deposit-detail__rows div { align-items: center; display: grid; grid-template-columns: 1fr auto; min-height: 62px; }.deposit-detail__rows dt { color: var(--ink); font-size: 16px; }.deposit-detail__rows dd { color: var(--muted-strong); font-size: 15px; margin: 0; text-align: right; }.deposit-detail__notice { background: #fff8e6; border-radius: var(--radius); color: #754c00; margin-top: 12px; padding: 14px; }.deposit-detail__notice strong { font-size: 13px; }.deposit-detail__notice p { font-size: 12px; line-height: 1.5; margin: 7px 0 0; }
</style>
