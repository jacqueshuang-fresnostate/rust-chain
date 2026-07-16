<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Info } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchDepositAssets, fetchDepositNetworks } from '@/api/wallet'
import { formatAmount } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { DepositNetwork } from '@/core/types'

const props = defineProps<{ asset: string }>()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const networks = ref<DepositNetwork[]>([])
const minimum = ref(0)
const error = ref('')
const loading = ref(false)

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const assets = await fetchDepositAssets()
    minimum.value = assets.find((asset) => asset.symbol === props.asset.toUpperCase())?.minDepositAmount || 0
    networks.value = await fetchDepositNetworks(props.asset, minimum.value)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('deposit.networkLoadFailed'))
  } finally {
    loading.value = false
  }
}

function chooseNetwork(network: DepositNetwork) {
  void router.push({ name: 'deposit-detail', params: { asset: props.asset, network: network.network } })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('deposit.selectNetwork')" />
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('deposit.networkLoginDescription')" />
      <template v-else>
        <section class="network-note"><Info :size="22" /><div><strong>{{ t('deposit.networkNoteTitle') }}</strong><p>{{ t('deposit.networkNoteDescription') }}</p></div></section>
        <div class="network-heading"><span>{{ t('deposit.network') }}</span><span>{{ t('deposit.networkHeading') }}</span></div>
        <p v-if="error" class="error-message">{{ error }}</p>
        <div class="network-list"><button v-for="network in networks" :key="network.network" type="button" @click="chooseNetwork(network)"><AssetMark :symbol="network.displayName" :size="40" /><strong>{{ network.displayName }}</strong><span><b>{{ t('deposit.estimatedMinutes', { minutes: network.estimatedMinutes }) }}</b><small>{{ formatAmount(network.minDepositAmount) }} {{ asset.toUpperCase() }}</small></span></button></div>
        <p v-if="!loading && !networks.length" class="empty-state">{{ t('deposit.noNetworks') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.network-note { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: flex; gap: 12px; margin: 16px 0 30px; padding: 15px; }.network-note svg { flex: 0 0 auto; }.network-note strong { font-size: 17px; }.network-note p { color: var(--muted-strong); font-size: 14px; line-height: 1.5; margin: 6px 0 0; }.network-heading { color: var(--muted); display: grid; font-size: 12px; grid-template-columns: 1fr auto; margin-bottom: 10px; }.network-list button { align-items: center; background: transparent; display: grid; gap: 13px; grid-template-columns: 40px 1fr auto; min-height: 83px; padding: 8px 0; text-align: left; width: 100%; }.network-list strong { font-size: 17px; }.network-list button > span { display: grid; text-align: right; }.network-list b { font-size: 15px; }.network-list small { color: var(--muted); font-size: 12px; margin-top: 5px; }
</style>
