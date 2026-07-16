<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ChevronRight, ReceiptText, Rocket } from 'lucide-vue-next'
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

function openProject(project: NewCoinProject): void {
  void router.push({ name: 'new-coin-detail', params: { symbol: project.symbol } })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain new-coins-page">
    <PageHeader :title="t('newCoin.title')">
      <template #actions><button class="icon-button" type="button" :aria-label="t('newCoin.records')" @click="router.push({ name: 'new-coin-records' })"><ReceiptText :size="20" /></button></template>
    </PageHeader>
    <div class="page-content">
      <p v-if="error" class="error-message">{{ error }}</p>
      <p v-if="loading" class="empty-state">{{ t('newCoin.loading') }}</p>
      <template v-else>
        <section class="new-coin-intro"><Rocket :size="25" /><div><strong>{{ t('newCoin.title') }}</strong><p>{{ t('newCoin.introDescription') }}</p></div></section>
        <div class="new-coin-list"><button v-for="project in projects" :key="project.id" type="button" @click="openProject(project)"><AssetMark :symbol="project.symbol" :size="42" /><div><strong>{{ project.symbol }}</strong><small>{{ lifecycleLabel(project.lifecycleStatus) }} · {{ unlockTypeLabel(project.unlockType) }}</small></div><span><b>{{ formatPrice(project.issuePrice) }}</b><small>{{ t('newCoin.issuePrice') }}</small></span><ChevronRight :size="18" /></button></div>
        <p v-if="!projects.length" class="empty-state">{{ t('newCoin.noProjects') }}</p>
        <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.loginDescription')" />
        <section v-else class="new-coin-history"><div class="section-heading"><span>{{ t('newCoin.recentSubscriptions') }}</span><button type="button" @click="router.push({ name: 'new-coin-records' })">{{ t('newCoin.allRecords') }}</button></div><article v-for="order in subscriptions.slice(0, 3)" :key="order.id"><div><strong>{{ t('newCoin.subscriptionUnits', { amount: formatAmount(order.requestedQuantity) }) }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div><span><b>{{ order.status }}</b><small>{{ t('newCoin.allocated', { amount: formatAmount(order.allocatedQuantity) }) }}</small></span></article><p v-if="!subscriptions.length" class="empty-state">{{ t('newCoin.noSubscriptions') }}</p></section>
      </template>
    </div>
  </main>
</template>

<style scoped>
.new-coins-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }
.new-coin-intro { align-items: center; background: #eef5ff; border: 1px solid #d5e5fd; border-radius: var(--radius); color: #3975ca; display: flex; gap: 11px; padding: 15px; }
.new-coin-intro div { display: grid; gap: 4px; }.new-coin-intro strong { color: var(--ink); font-size: 17px; }.new-coin-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.4; margin: 0; }
.new-coin-list { display: grid; gap: 10px; }.new-coin-list button { align-items: center; background: white; border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 12px; grid-template-columns: 42px minmax(0, 1fr) auto 18px; min-height: 80px; padding: 12px; text-align: left; width: 100%; }.new-coin-list button > div,.new-coin-list button > span { display: grid; gap: 5px; min-width: 0; }.new-coin-list strong { font-size: 16px; }.new-coin-list small { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.new-coin-list > span { text-align: right; }.new-coin-list > span b { font-size: 15px; }.new-coin-list > svg { color: var(--muted); }
.new-coin-history { border-top: 1px solid var(--line); }.new-coin-history .section-heading { margin-top: 20px; }.new-coin-history .section-heading button { background: transparent; color: var(--accent); font-size: 12px; font-weight: 700; }.new-coin-history article { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 62px; }.new-coin-history article div,.new-coin-history article > span { display: grid; gap: 5px; }.new-coin-history strong,.new-coin-history b { font-size: 13px; }.new-coin-history small { color: var(--muted); font-size: 11px; }.new-coin-history article > span { text-align: right; }
</style>
