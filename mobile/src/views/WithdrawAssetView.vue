<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CircleHelp, Search } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchWithdrawalAssets, type WithdrawalAsset } from '@/api/wallet'
import { formatAmount } from '@/core/format'
import { useSessionStore } from '@/stores/session'

const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const assets = ref<WithdrawalAsset[]>([])
const query = ref('')
const loading = ref(false)
const error = ref('')

const filteredAssets = computed(() => {
  const keyword = query.value.trim().toUpperCase()
  return keyword ? assets.value.filter((asset) => `${asset.symbol}${asset.name || ''}`.toUpperCase().includes(keyword)) : assets.value
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    assets.value = await fetchWithdrawalAssets()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('withdraw.assetLoadFailed'))
  } finally {
    loading.value = false
  }
}

function selectAsset(asset: WithdrawalAsset): void {
  void router.push({ name: 'withdraw', params: { asset: asset.symbol } })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('withdraw.selectAsset')">
      <template #actions><button class="icon-button" type="button" :aria-label="t('withdraw.help')"><CircleHelp :size="22" /></button></template>
    </PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('withdraw.assetLoginDescription')" />
      <template v-else>
        <label class="asset-search"><Search :size="22" /><input v-model="query" type="search" :placeholder="t('withdraw.searchPlaceholder')" /></label>
        <div class="section-heading"><span>{{ t('withdraw.availableAssets') }}</span></div>
        <p v-if="error" class="error-message">{{ error }}</p>
        <div class="asset-picker">
          <button v-for="asset in filteredAssets" :key="asset.symbol" type="button" @click="selectAsset(asset)">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="44" />
            <span><b>{{ asset.symbol }}</b><small>{{ asset.name || t('withdraw.onchain') }}</small></span>
            <em>{{ t('withdraw.feeLabel', { amount: formatAmount(asset.withdrawFee) }) }}</em>
          </button>
        </div>
        <p v-if="!loading && !filteredAssets.length" class="empty-state">{{ t('withdraw.noAssets') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.asset-search { align-items: center; background: var(--soft); border-radius: 26px; color: var(--ink); display: flex; gap: 11px; min-height: 52px; padding: 0 16px; }.asset-search input { background: transparent; border: 0; font-size: 16px; outline: 0; width: 100%; }
.asset-picker { display: grid; }.asset-picker button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: grid; gap: 14px; grid-template-columns: 44px minmax(0, 1fr) auto; min-height: 82px; padding: 8px 0; text-align: left; width: 100%; }.asset-picker button > span { display: grid; min-width: 0; }.asset-picker b { font-size: 18px; }.asset-picker small,.asset-picker em { color: var(--muted); font-size: 13px; font-style: normal; margin-top: 5px; }.asset-picker em { margin: 0; text-align: right; }
</style>
