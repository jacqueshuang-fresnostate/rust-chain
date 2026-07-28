<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronRight, LoaderCircle, Search } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchDepositAssets } from '@/api/wallet'
import { useSessionStore } from '@/stores/session'
import type { DepositAsset } from '@/core/types'

const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const assets = ref<DepositAsset[]>([])
const query = ref('')
const loading = ref(false)
const error = ref('')
const filteredAssets = computed(() => {
  const keyword = query.value.trim().toUpperCase()
  return keyword ? assets.value.filter((item) => item.symbol.includes(keyword)) : assets.value
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try { assets.value = await fetchDepositAssets() } catch (reason) { error.value = apiErrorMessage(reason, t('deposit.assetLoadFailed')) } finally { loading.value = false }
}

function selectAsset(asset: DepositAsset) {
  void router.push({ name: 'deposit-network', params: { asset: asset.symbol } })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader
      :back="true"
      :eyebrow="t('assets.deposit')"
      :subtitle="t('deposit.searchPlaceholder')"
      :title="t('deposit.selectAsset')"
    />
    <div class="page-content asset-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('deposit.assetLoginDescription')" />
      <template v-else>
        <label class="asset-search">
          <Search :size="20" aria-hidden="true" />
          <input v-model="query" type="search" :aria-label="t('deposit.searchPlaceholder')" :placeholder="t('deposit.searchPlaceholder')" />
        </label>
        <div class="section-heading asset-heading"><span>{{ t('deposit.crypto') }}</span></div>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="loading-state" role="status" :aria-label="t('common.loading')">
          <LoaderCircle :size="22" class="spin" aria-hidden="true" />
          <span>{{ t('common.loading') }}</span>
        </div>
        <div v-else-if="filteredAssets.length" class="asset-picker">
          <button v-for="asset in filteredAssets" :key="asset.symbol" type="button" @click="selectAsset(asset)">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="44" />
            <span>
              <b>{{ asset.symbol }}</b>
              <small>{{ t('deposit.supported') }}</small>
            </span>
            <ChevronRight :size="19" aria-hidden="true" />
          </button>
        </div>
        <p v-else class="empty-state">{{ t('deposit.noAssets') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.asset-page {
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.asset-search {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  color: var(--muted);
  display: flex;
  gap: 10px;
  min-height: 50px;
  padding: 0 14px;
}

.asset-search:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
  color: var(--ink);
}

.asset-search input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 15px;
  min-height: 48px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.asset-heading {
  border-bottom: 1px solid var(--line);
  font-size: 14px;
  margin: 26px 0 0;
  padding-bottom: 10px;
}

.loading-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 9px;
  justify-content: center;
  min-height: 104px;
}

.asset-picker {
  display: grid;
}

.asset-picker button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  color: var(--ink);
  display: grid;
  gap: 14px;
  grid-template-columns: 44px minmax(0, 1fr) 24px;
  min-height: 78px;
  padding: 8px 2px;
  text-align: left;
  width: 100%;
}

.asset-picker button:hover {
  background: var(--surface-elevated);
}

.asset-picker button > span {
  display: grid;
  min-width: 0;
}

.asset-picker b {
  font-size: 17px;
}

.asset-picker small {
  color: var(--muted);
  font-size: 12px;
  margin-top: 4px;
}

.asset-picker svg {
  color: var(--muted);
  justify-self: end;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .asset-picker button {
    gap: 10px;
    grid-template-columns: 40px minmax(0, 1fr) 20px;
  }
}
</style>
