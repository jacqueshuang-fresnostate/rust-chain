<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ChevronRight,
  CircleAlert,
  History,
  LoaderCircle,
  PackageOpen,
  ReceiptText,
  RefreshCw,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
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
const lifecycleFilter = ref<'all' | 'active' | 'closed'>('all')
const visibleProjects = computed(() => projects.value.filter((project) => {
  const status = project.lifecycleStatus.toLowerCase()
  if (lifecycleFilter.value === 'active') return ['subscription', 'distribution', 'listed'].includes(status)
  if (lifecycleFilter.value === 'closed') return status === 'closed'
  return true
}))

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

function lifecycleStep(status: string): number {
  return ({ subscription: 1, distribution: 2, listed: 3, closed: 4 } as Record<string, number>)[status.toLowerCase()] || 0
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
  <main class="page page--plain pencil-page new-coins-pencil" data-pencil-source="oOJ0q ZTtvY">
    <PageHeader :back="true" :pencil="true" :title="t('newCoin.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('newCoin.records')" @click="router.push({ name: 'new-coin-records' })"><History :size="18" /></button>
      </template>
    </PageHeader>

    <div class="pencil-content new-coins-pencil__content">
      <section class="new-coins-hero">
        <h1>{{ t('newCoin.heroTitle') }}</h1>
        <p>{{ t('newCoin.heroDescription') }}</p>
      </section>

      <nav class="pencil-segmented pencil-segmented--soft new-coins-tabs" :aria-label="t('newCoin.projectFilters')">
        <button type="button" :aria-pressed="lifecycleFilter === 'all'" @click="lifecycleFilter = 'all'">{{ t('common.all') }}</button>
        <button type="button" :aria-pressed="lifecycleFilter === 'active'" @click="lifecycleFilter = 'active'">{{ t('newCoin.inProgress') }}</button>
        <button type="button" :aria-pressed="lifecycleFilter === 'closed'" @click="lifecycleFilter = 'closed'">{{ t('newCoin.ended') }}</button>
      </nav>

      <div v-if="error" class="pencil-message pencil-message--error new-coin-project-state" role="alert">
        <CircleAlert :size="18" /><span>{{ error }}</span>
        <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="17" /></button>
      </div>
      <div v-else-if="loading" class="pencil-state new-coin-project-state" aria-live="polite"><LoaderCircle :size="24" class="spin" /><span>{{ t('newCoin.loading') }}</span></div>

      <div v-else-if="visibleProjects.length" class="new-coin-projects">
        <button v-for="project in visibleProjects" :key="project.id" class="new-coin-project" type="button" @click="openProject(project)">
          <header>
            <AssetMark :symbol="project.symbol" :size="42" />
            <span><strong>{{ project.symbol }}</strong><small>{{ unlockTypeLabel(project.unlockType) }}</small></span>
            <b class="pencil-pill" :class="{ 'pencil-pill--negative': project.lifecycleStatus.toLowerCase() === 'closed' }">{{ lifecycleLabel(project.lifecycleStatus) }}</b>
          </header>
          <div class="new-coin-stage" :aria-label="lifecycleLabel(project.lifecycleStatus)">
            <i :style="{ width: `${lifecycleStep(project.lifecycleStatus) * 25}%` }" />
          </div>
          <footer>
            <dl>
              <div><dt>{{ t('newCoin.issuePrice') }}</dt><dd class="pencil-numeric">{{ formatPrice(project.issuePrice) }}</dd></div>
              <div><dt>{{ t('newCoin.plannedIssue') }}</dt><dd class="pencil-numeric">{{ formatAmount(project.totalSupply) }} {{ project.symbol }}</dd></div>
            </dl>
            <span class="new-coin-project__action">{{ t('newCoin.viewDetails') }}<ChevronRight :size="16" /></span>
          </footer>
        </button>
      </div>
      <div v-else class="pencil-state new-coin-project-state"><PackageOpen :size="23" /><span>{{ t('newCoin.noProjects') }}</span></div>

      <button class="new-coin-records-link" type="button" @click="router.push({ name: 'new-coin-records' })">
        <ReceiptText :size="18" />
        <span>{{ t('newCoin.records') }}</span>
        <ChevronRight :size="17" />
      </button>

      <section v-if="session.isAuthenticated" class="pencil-section new-coin-records-pencil">
        <div class="pencil-section__heading">
          <h2>{{ t('newCoin.recentSubscriptions') }}</h2>
          <button type="button" @click="router.push({ name: 'new-coin-records' })">{{ t('newCoin.allRecords') }}</button>
        </div>
        <div v-if="subscriptions.length" class="pencil-list">
          <article v-for="order in subscriptions.slice(0, 3)" :key="order.id" class="pencil-row new-coin-record-row">
            <span class="pencil-row__icon"><ReceiptText :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('newCoin.subscriptionUnits', { amount: formatAmount(order.requestedQuantity) }) }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></span>
            <span class="pencil-row__value"><strong>{{ statusLabel(order.status) }}</strong><small>{{ t('newCoin.allocated', { amount: formatAmount(order.allocatedQuantity) }) }}</small></span>
          </article>
        </div>
        <div v-else class="pencil-state"><PackageOpen :size="22" /><span>{{ t('newCoin.noSubscriptions') }}</span></div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.new-coins-pencil__content {
  min-height: 461px;
  padding-top: 0;
}

.new-coins-hero {
  height: 72px;
  padding-top: 8px;
}

.new-coins-hero h1 {
  font-size: 22px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 32px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coins-hero p {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  margin: 16px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coins-tabs {
  height: 30px;
  margin-top: 17px;
  min-height: 30px;
  overflow: visible;
}

.new-coins-tabs button {
  height: 30px;
  min-height: 30px;
}

.new-coins-tabs button::before {
  inset: -7px 0;
}

.new-coin-projects {
  display: grid;
  gap: 16px;
  padding-top: 16px;
}

.new-coin-project {
  background: transparent;
  border: 0;
  border-radius: 0;
  color: var(--ink);
  display: block;
  height: 113px;
  padding: 15px 0 0;
  text-align: left;
  width: 100%;
}

.new-coin-project:focus-visible,
.new-coin-records-link:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.new-coin-project > header {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: 42px minmax(0, 1fr) auto;
  height: 42px;
}

.new-coin-project > header > span {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.new-coin-project > header strong {
  font-size: 15px;
  line-height: 20px;
}

.new-coin-project > header small {
  color: var(--muted);
  font-size: 9px;
  line-height: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-stage {
  background: var(--accent-soft);
  border-radius: 999px;
  height: 4px;
  margin-top: 10px;
  overflow: hidden;
}

.new-coin-stage i {
  background: var(--accent);
  border-radius: inherit;
  display: block;
  height: 4px;
}

.new-coin-project > footer {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 34px;
  margin-top: 8px;
}

.new-coin-project dl {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.new-coin-project dl > div {
  display: grid;
  gap: 1px;
  min-width: 0;
}

.new-coin-project dt {
  color: var(--muted);
  font-size: 8px;
  line-height: 11px;
}

.new-coin-project dd {
  font-size: 9px;
  line-height: 13px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project__action {
  align-items: center;
  color: var(--positive);
  display: inline-flex;
  font-size: 10px;
  font-weight: 700;
  gap: 3px;
  justify-self: end;
  white-space: nowrap;
}

.new-coin-project-state {
  height: 242px;
  margin-top: 16px;
  min-height: 242px;
}

.new-coin-records-link {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 18px minmax(0, 1fr) 18px;
  height: 48px;
  margin-top: 16px;
  padding: 0;
  text-align: left;
  width: 100%;
}

.new-coin-records-link span {
  font-size: 12px;
  font-weight: 500;
}

.new-coin-records-link > svg:last-child {
  color: var(--muted);
}

.new-coin-records-pencil {
  margin-top: 16px;
}

.new-coin-record-row {
  grid-template-columns: 40px minmax(0, 1fr) auto;
}

.new-coin-record-row .pencil-row__value strong {
  font-size: 10px;
}

.new-coins-pencil :deep(.asset-mark) {
  --asset-color: var(--accent);
  --asset-ink: var(--on-accent);
  background: var(--accent);
  border: 0;
  box-shadow: none;
  color: var(--on-accent);
}

@media (max-width: 340px) {
  .new-coin-project > header {
    gap: 8px;
  }

  .new-coin-project dl {
    gap: 6px;
  }

  .new-coin-project__action {
    font-size: 9px;
  }
}
</style>
