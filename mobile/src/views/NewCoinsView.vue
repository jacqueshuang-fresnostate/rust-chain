<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ChevronRight,
  CircleAlert,
  LoaderCircle,
  PackageOpen,
  ReceiptText,
  RefreshCw,
  Rocket,
  ShieldCheck,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNewCoinProjects, fetchNewCoinSubscriptions, type NewCoinProject, type NewCoinSubscription } from '@/api/newCoin'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useSessionStore } from '@/stores/session'

const router = useRouter()
const { t } = useI18n()
const session = useSessionStore()
const projects = ref<NewCoinProject[]>([])
const subscriptions = ref<NewCoinSubscription[]>([])
const loading = ref(false)
const error = ref('')

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const projectPromise = fetchNewCoinProjects()
    if (session.isAuthenticated) {
      const [nextProjects, nextSubscriptions] = await Promise.all([projectPromise, fetchNewCoinSubscriptions()])
      projects.value = nextProjects
      subscriptions.value = nextSubscriptions
    } else {
      projects.value = await projectPromise
      subscriptions.value = []
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.projectLoadFailed'))
  } finally {
    loading.value = false
  }
}

function lifecycleLabel(status: string): string {
  const keys: Record<string, string> = {
    subscription: 'newCoin.subscriptionOpen',
    distribution: 'newCoin.waitingDistribution',
    listed: 'newCoin.listed',
    closed: 'newCoin.closed',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function unlockTypeLabel(type: string): string {
  const keys: Record<string, string> = {
    fixed: 'newCoin.fixedUnlock',
    relative: 'newCoin.relativeUnlock',
  }
  const key = keys[type.toLowerCase()]
  return key ? t(key) : type || t('newCoin.unlockPending')
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

function openProject(project: NewCoinProject): void {
  void router.push({ name: 'new-coin-detail', params: { symbol: project.symbol } })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain new-coins-page">
    <PageHeader
      :back="true"
      :eyebrow="t('products.newCoins')"
      :subtitle="t('newCoin.introDescription')"
      :title="t('newCoin.title')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('newCoin.records')"
          @click="router.push({ name: 'new-coin-records' })"
        >
          <ReceiptText :size="20" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content new-coins-content">
      <div v-if="error" class="new-coin-message" role="alert">
        <CircleAlert :size="18" />
        <span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load">
          <RefreshCw :size="17" />
        </button>
      </div>
      <div v-if="loading" class="new-coin-state" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <span>{{ t('newCoin.loading') }}</span>
      </div>
      <template v-else>
        <section class="new-coin-overview">
          <div class="new-coin-overview__icon"><Rocket :size="23" /></div>
          <div>
            <strong>{{ t('newCoin.title') }}</strong>
            <p>{{ t('newCoin.introDescription') }}</p>
          </div>
          <ShieldCheck :size="20" />
        </section>

        <div v-if="projects.length" class="new-coin-list">
          <button v-for="project in projects" :key="project.id" type="button" @click="openProject(project)">
            <AssetMark :symbol="project.symbol" :size="42" />
            <div>
              <strong>{{ project.symbol }}</strong>
              <small>{{ lifecycleLabel(project.lifecycleStatus) }} · {{ unlockTypeLabel(project.unlockType) }}</small>
            </div>
            <span>
              <b class="numeric">{{ formatPrice(project.issuePrice) }}</b>
              <small>{{ t('newCoin.issuePrice') }}</small>
            </span>
            <ChevronRight :size="18" />
          </button>
        </div>
        <div v-else class="new-coin-state new-coin-state--empty">
          <PackageOpen :size="23" />
          <span>{{ t('newCoin.noProjects') }}</span>
        </div>

        <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.loginDescription')" />
        <section v-else class="new-coin-history">
          <div class="section-heading">
            <span>{{ t('newCoin.recentSubscriptions') }}</span>
            <button type="button" @click="router.push({ name: 'new-coin-records' })">{{ t('newCoin.allRecords') }}</button>
          </div>
          <article v-for="order in subscriptions.slice(0, 3)" :key="order.id">
            <div>
              <strong>{{ t('newCoin.subscriptionUnits', { amount: formatAmount(order.requestedQuantity) }) }}</strong>
              <small>{{ formatDateTime(order.createdAt) }}</small>
            </div>
            <span>
              <b>{{ statusLabel(order.status) }}</b>
              <small>{{ t('newCoin.allocated', { amount: formatAmount(order.allocatedQuantity) }) }}</small>
            </span>
          </article>
          <div v-if="!subscriptions.length" class="new-coin-state new-coin-state--empty">
            <PackageOpen :size="22" />
            <span>{{ t('newCoin.noSubscriptions') }}</span>
          </div>
        </section>
      </template>
    </div>
  </main>
</template>

<style scoped>
.new-coins-page {
  background: var(--surface);
  min-width: 0;
}

.new-coins-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.new-coin-message {
  align-items: center;
  background: var(--negative-soft);
  border: 1px solid var(--negative);
  color: var(--negative);
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 1.45;
  min-height: 52px;
  padding: 4px 5px 4px 11px;
}

.new-coin-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.new-coin-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 148px;
  text-align: center;
}

.new-coin-state--empty {
  min-height: 112px;
}

.new-coin-overview {
  align-items: center;
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--accent) 9%, transparent), transparent 64%),
    var(--surface);
  border-block: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 11px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  min-height: 92px;
  padding: 12px 4px;
}

.new-coin-overview__icon {
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  color: var(--accent);
  display: grid;
  height: 44px;
  place-items: center;
  width: 44px;
}

.new-coin-overview > div:nth-child(2) {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.new-coin-overview strong {
  font-size: 17px;
}

.new-coin-overview p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.new-coin-overview > svg {
  color: var(--positive);
}

.new-coin-list {
  border-block: 1px solid var(--line);
  display: grid;
}

.new-coin-list button {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 11px;
  grid-template-columns: 42px minmax(0, 1fr) auto 18px;
  min-height: 78px;
  min-width: 0;
  padding: 10px 4px;
  text-align: left;
  width: 100%;
}

.new-coin-list button:last-child {
  border-bottom: 0;
}

.new-coin-list button:focus-visible,
.new-coin-list button:hover {
  background: var(--surface-elevated);
  box-shadow: inset 3px 0 0 var(--accent);
}

.new-coin-list button > div,
.new-coin-list button > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.new-coin-list strong {
  font-size: 15px;
}

.new-coin-list small {
  color: var(--muted);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-list button > span {
  text-align: right;
}

.new-coin-list button > span b {
  font-size: 14px;
}

.new-coin-list button > svg {
  color: var(--muted);
}

.new-coin-history {
  border-top: 8px solid var(--soft);
  margin: 8px -20px 0;
  padding: 0 20px;
}

.new-coin-history .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 56px;
}

.new-coin-history .section-heading button {
  background: transparent;
  color: var(--accent);
  font-size: 11px;
  font-weight: 800;
  min-height: 44px;
  padding: 0;
}

.new-coin-history article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 68px;
}

.new-coin-history article > div,
.new-coin-history article > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.new-coin-history strong,
.new-coin-history b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.new-coin-history small {
  color: var(--muted);
  font-size: 10px;
}

.new-coin-history article > span {
  flex: 0 0 auto;
  text-align: right;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .new-coins-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .new-coin-history {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .new-coin-overview {
    grid-template-columns: 40px minmax(0, 1fr);
  }

  .new-coin-overview__icon {
    height: 40px;
    width: 40px;
  }

  .new-coin-overview > svg {
    display: none;
  }

  .new-coin-list button {
    gap: 8px;
    grid-template-columns: 38px minmax(0, 1fr) 16px;
  }

  .new-coin-list button > span {
    grid-column: 2;
    grid-row: 2;
    justify-items: start;
    text-align: left;
  }

  .new-coin-list button > svg {
    grid-column: 3;
    grid-row: 1 / span 2;
  }

  .new-coin-history article {
    align-items: flex-start;
    flex-direction: column;
    padding: 11px 0;
  }

  .new-coin-history article > span {
    align-items: center;
    display: flex;
    justify-content: space-between;
    width: 100%;
  }
}
</style>
