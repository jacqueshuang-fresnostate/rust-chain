<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Info, LoaderCircle } from 'lucide-vue-next'
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
    <PageHeader
      :back="true"
      :eyebrow="t('assets.deposit')"
      :subtitle="t('deposit.networkNoteDescription')"
      :title="t('deposit.selectNetwork')"
    />
    <div class="page-content network-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('deposit.networkLoginDescription')" />
      <template v-else>
        <section class="network-note">
          <Info :size="21" aria-hidden="true" />
          <div>
            <strong>{{ t('deposit.networkNoteTitle') }}</strong>
            <p>{{ t('deposit.networkNoteDescription') }}</p>
          </div>
        </section>
        <div class="network-heading"><span>{{ t('deposit.network') }}</span><span>{{ t('deposit.networkHeading') }}</span></div>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="loading-state" role="status" :aria-label="t('common.loading')">
          <LoaderCircle :size="22" class="spin" aria-hidden="true" />
          <span>{{ t('common.loading') }}</span>
        </div>
        <div v-else-if="networks.length" class="network-list">
          <button v-for="network in networks" :key="network.network" type="button" @click="chooseNetwork(network)">
            <AssetMark :symbol="network.displayName" :size="40" />
            <strong>{{ network.displayName }}</strong>
            <span>
              <b>{{ t('deposit.estimatedMinutes', { minutes: network.estimatedMinutes }) }}</b>
              <small>{{ formatAmount(network.minDepositAmount) }} {{ asset.toUpperCase() }}</small>
            </span>
          </button>
        </div>
        <p v-else class="empty-state">{{ t('deposit.noNetworks') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.network-page {
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.network-note {
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: var(--radius);
  display: flex;
  gap: 12px;
  margin: 0 0 28px;
  padding: 14px;
}

.network-note svg {
  color: var(--accent);
  flex: 0 0 auto;
  margin-top: 1px;
}

.network-note strong {
  font-size: 15px;
}

.network-note p {
  color: var(--muted-strong);
  font-size: 13px;
  line-height: 1.5;
  margin: 5px 0 0;
}

.network-heading {
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  display: grid;
  font-size: 11px;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) auto;
  padding-bottom: 9px;
}

.network-heading span:last-child {
  text-align: right;
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

.network-list {
  display: grid;
}

.network-list button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 40px minmax(0, 1fr) minmax(116px, auto);
  min-height: 78px;
  padding: 8px 2px;
  text-align: left;
  width: 100%;
}

.network-list button:hover {
  background: var(--surface-elevated);
}

.network-list strong {
  font-size: 16px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.network-list button > span {
  display: grid;
  min-width: 0;
  text-align: right;
}

.network-list b {
  font-size: 13px;
  white-space: nowrap;
}

.network-list small {
  color: var(--muted);
  font-size: 11px;
  margin-top: 4px;
  white-space: nowrap;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .network-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .network-heading {
    font-size: 10px;
  }

  .network-list button {
    gap: 9px;
    grid-template-columns: 36px minmax(0, 1fr) minmax(104px, auto);
  }

  .network-list strong {
    font-size: 14px;
  }
}
</style>
