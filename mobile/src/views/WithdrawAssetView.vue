<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronRight, LoaderCircle, Search } from 'lucide-vue-next'
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
  <main
    class="page page--plain pencil-page wallet-pencil-page withdraw-asset-pencil"
    data-pencil-source="NGBmq h0WWYC"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.withdraw')"
      :fallback="{ name: 'assets' }"
      :pencil="true"
      :subtitle="t('withdraw.searchPlaceholder')"
      :title="t('withdraw.selectAsset')"
    />
    <div class="page-content asset-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('withdraw.assetLoginDescription')"
      />
      <template v-else>
        <label class="asset-search">
          <Search :size="16" aria-hidden="true" />
          <input v-model="query" type="search" :aria-label="t('withdraw.searchPlaceholder')" :placeholder="t('withdraw.searchPlaceholder')" />
        </label>
        <p v-if="error" class="error-message wallet-feedback" role="alert">{{ error }}</p>
        <div v-if="loading" class="loading-state" role="status" :aria-label="t('common.loading')">
          <LoaderCircle :size="22" class="spin" aria-hidden="true" />
          <span>{{ t('common.loading') }}</span>
        </div>
        <div v-else-if="filteredAssets.length" class="asset-picker">
          <button v-for="asset in filteredAssets" :key="asset.symbol" type="button" @click="selectAsset(asset)">
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="36" />
            <span>
              <b>{{ asset.symbol }}</b>
              <small>{{ asset.name || t('withdraw.onchain') }} · {{ t('withdraw.feeLabel', { amount: formatAmount(asset.withdrawFee) }) }}</small>
            </span>
            <ChevronRight :size="16" aria-hidden="true" />
          </button>
        </div>
        <p v-else class="empty-state">{{ t('withdraw.noAssets') }}</p>
        <p class="asset-note">{{ t('withdraw.notice') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.asset-page {
  display: grid;
  gap: 12px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.asset-search {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--muted);
  display: flex;
  gap: 8px;
  height: 44px;
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
  font-size: 12px;
  min-height: 42px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.asset-search input::placeholder {
  color: var(--muted);
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
  border-bottom: 1px solid var(--hairline);
  border-radius: 0;
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 36px minmax(0, 1fr) 16px;
  height: 60px;
  min-height: 60px;
  padding: 0;
  text-align: left;
  width: 100%;
}

.asset-picker button:hover {
  background: var(--surface-elevated);
}

.asset-picker button > span {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.asset-picker b {
  font-size: 14px;
  line-height: 20px;
}

.asset-picker small {
  color: var(--muted);
  font-size: 11px;
  line-height: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-picker svg {
  color: var(--muted);
  justify-self: end;
}

.asset-note {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
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
  .asset-page {
    padding-inline: 16px;
  }

  .asset-picker button {
    gap: 9px;
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
