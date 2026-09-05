<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ChevronRight, CircleAlert, LoaderCircle, PackageOpen, ReceiptText, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import NewCoinOpportunityCard from '@/components/new-coin/NewCoinOpportunityCard.vue'
import NewCoinProjectCard from '@/components/new-coin/NewCoinProjectCard.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNewCoinProjects, type NewCoinProject } from '@/api/newCoin'
import {
  buildNewCoinOpportunities,
  filterNewCoinProjects,
  type NewCoinLifecycleFilter,
  type NewCoinOpportunity,
  type NewCoinOpportunityFilter,
} from '@/core/newCoinPresentation'
import { useMarketStore } from '@/stores/market'
import launchBannerUrl from '@/assets/new-coin-launch-banner.jpg'

type PrimaryTab = 'activities' | 'opportunities'

const MARKET_CONSUMER_ID = 'new-coin-opportunities-view'
const router = useRouter()
const { t } = useI18n()
const market = useMarketStore()
const projects = ref<NewCoinProject[]>([])
const primaryTab = ref<PrimaryTab>('activities')
const lifecycleFilter = ref<NewCoinLifecycleFilter>('all')
const opportunityFilter = ref<NewCoinOpportunityFilter>('all')
const loading = ref(false)
const projectError = ref('')
const now = ref(Date.now())
let clock: ReturnType<typeof setInterval> | undefined
let marketLeaseHeld = false
let marketInitialized = false

const lifecycleFilters: ReadonlyArray<{ key: NewCoinLifecycleFilter; label: string }> = [
  { key: 'all', label: 'common.all' },
  { key: 'preheat', label: 'newCoin.preheat' },
  { key: 'subscription', label: 'newCoin.subscribe' },
  { key: 'distribution', label: 'newCoin.pendingListing' },
  { key: 'listed', label: 'newCoin.listed' },
]
const opportunityFilters: ReadonlyArray<{ key: NewCoinOpportunityFilter; label: string }> = [
  { key: 'all', label: 'common.all' },
  { key: 'upcoming', label: 'newCoin.upcoming' },
  { key: 'listedToday', label: 'newCoin.listedToday' },
  { key: 'hotGains', label: 'newCoin.hotGains' },
]
const visibleProjects = computed(() => filterNewCoinProjects(projects.value, lifecycleFilter.value))
const opportunities = computed(() => buildNewCoinOpportunities(
  projects.value,
  market.tickers,
  opportunityFilter.value,
  now.value,
))
const showProjectError = computed(() => Boolean(projectError.value && !projects.value.length))
const showProjectWarning = computed(() => Boolean(projectError.value && projects.value.length))
const showMarketError = computed(() => primaryTab.value === 'opportunities'
  && market.error
  && !opportunities.value.length)

async function loadProjects(force = false): Promise<void> {
  loading.value = !projects.value.length
  projectError.value = ''
  try {
    projects.value = await fetchNewCoinProjects(50, { force })
  } catch (reason) {
    projectError.value = apiErrorMessage(reason, t('newCoin.projectLoadFailed'))
  } finally {
    loading.value = false
  }
}

function openProject(project: NewCoinProject): void {
  void router.push({ name: 'new-coin-detail', params: { symbol: project.symbol } })
}

function openTrade(opportunity: NewCoinOpportunity): void {
  void router.push({
    name: 'trade',
    params: { symbol: opportunity.ticker.symbol.replace('/', '_') },
  })
}

function retryMarket(): void {
  void market.refresh(true)
}

function initializeMarket(): void {
  if (marketInitialized) return
  marketInitialized = true
  void market.refresh()
}

function syncMarketLease(tab: PrimaryTab): void {
  if (tab === 'opportunities' && !marketLeaseHeld) {
    initializeMarket()
    market.startLiveUpdates(MARKET_CONSUMER_ID)
    marketLeaseHeld = true
  } else if (tab !== 'opportunities' && marketLeaseHeld) {
    market.stopLiveUpdates(MARKET_CONSUMER_ID)
    marketLeaseHeld = false
  }
}

watch(primaryTab, syncMarketLease)

onMounted(() => {
  void loadProjects()
  syncMarketLease(primaryTab.value)
  clock = setInterval(() => { now.value = Date.now() }, 30_000)
})

onBeforeUnmount(() => {
  if (clock) clearInterval(clock)
  if (marketLeaseHeld) market.stopLiveUpdates(MARKET_CONSUMER_ID)
})
</script>

<template>
  <main
    class="page page--plain pencil-page new-coins-pencil"
    data-pencil-source="oOJ0q ZTtvY XG67j E2qzxN"
  >
    <PageHeader :back="true" :pencil="true" back-icon="chevron" :title="t('newCoin.title')">
      <template #actions>
        <button
          class="icon-button new-coins-pencil__records"
          type="button"
          :aria-label="t('newCoin.records')"
          @click="router.push({ name: 'new-coin-records' })"
        >
          <ReceiptText :size="23" />
        </button>
      </template>
    </PageHeader>

    <section class="new-coins-banner" :aria-label="t('newCoin.bannerLabel')">
      <img :src="launchBannerUrl" alt="" />
    </section>

    <nav class="new-coins-primary-tabs" :aria-label="t('newCoin.primaryTabs')">
      <button type="button" :aria-pressed="primaryTab === 'activities'" @click="primaryTab = 'activities'">
        {{ t('newCoin.activities') }}
      </button>
      <button type="button" :aria-pressed="primaryTab === 'opportunities'" @click="primaryTab = 'opportunities'">
        {{ t('newCoin.opportunities') }}
      </button>
    </nav>

    <template v-if="primaryTab === 'activities'">
      <nav class="new-coins-lifecycle-filters" :aria-label="t('newCoin.projectFilters')">
        <button
          v-for="filter in lifecycleFilters"
          :key="filter.key"
          type="button"
          :aria-pressed="lifecycleFilter === filter.key"
          @click="lifecycleFilter = filter.key"
        >
          <span>{{ t(filter.label) }}</span>
        </button>
      </nav>

      <section class="new-coins-project-content">
        <div v-if="showProjectWarning" class="new-coins-inline-warning" role="status">
          <CircleAlert :size="15" />
          <span>{{ projectError }}</span>
          <button type="button" :aria-label="t('common.retry')" @click="loadProjects(true)"><RefreshCw :size="15" /></button>
        </div>
        <div v-if="showProjectError" class="new-coins-state" role="alert">
          <CircleAlert :size="24" />
          <span>{{ projectError }}</span>
          <button type="button" @click="loadProjects(true)">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="loading" class="new-coins-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('newCoin.loading') }}</span>
        </div>
        <template v-else-if="visibleProjects.length">
          <h2>{{ t('newCoin.featuredProjects') }}<ChevronRight :size="20" aria-hidden="true" /></h2>
          <div class="new-coins-project-list">
            <NewCoinProjectCard
              v-for="project in visibleProjects"
              :key="project.id"
              :project="project"
              :now="now"
              @open="openProject(project)"
            />
          </div>
        </template>
        <div v-else class="new-coins-state">
          <PackageOpen :size="24" />
          <span>{{ t('newCoin.noProjects') }}</span>
        </div>
      </section>
    </template>

    <section v-else class="new-coins-opportunity-content">
      <nav class="new-coins-opportunity-filters" :aria-label="t('newCoin.opportunityFilters')">
        <button
          v-for="filter in opportunityFilters"
          :key="filter.key"
          type="button"
          :aria-pressed="opportunityFilter === filter.key"
          @click="opportunityFilter = filter.key"
        >
          <span>{{ t(filter.label) }}</span>
        </button>
      </nav>

      <div v-if="showMarketError" class="new-coins-state" role="alert">
        <CircleAlert :size="24" />
        <span>{{ t('newCoin.marketLoadFailed') }}</span>
        <button type="button" @click="retryMarket">{{ t('common.retry') }}</button>
      </div>
      <div v-else-if="market.loading && !market.tickers.length" class="new-coins-state" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <span>{{ t('newCoin.loadingOpportunities') }}</span>
      </div>
      <div v-else-if="opportunities.length" class="new-coins-opportunity-list">
        <NewCoinOpportunityCard
          v-for="opportunity in opportunities"
          :key="opportunity.project.id"
          :opportunity="opportunity"
          :now="now"
          @trade="openTrade(opportunity)"
        />
      </div>
      <div v-else class="new-coins-state">
        <PackageOpen :size="24" />
        <span>{{ t('newCoin.noOpportunities') }}</span>
      </div>
    </section>
  </main>
</template>

<style scoped>
.new-coins-pencil {
  min-height: 100dvh;
  overflow-x: clip;
  padding-bottom: calc(18px + env(safe-area-inset-bottom));
}

.new-coins-pencil :deep(.pencil-page-header) {
  background: var(--new-coin-header);
  height: 54px;
  min-height: 54px;
  padding: 5px 16px;
}

.new-coins-pencil :deep(.page-header__title) {
  font-size: 21px;
  font-weight: 700;
  line-height: 30px;
}

.new-coins-banner {
  box-sizing: border-box;
  height: 148px;
  padding: 8px 16px;
}

.new-coins-banner img {
  border-radius: 14px;
  display: block;
  height: 132px;
  object-fit: cover;
  width: 100%;
}

.new-coins-primary-tabs {
  align-items: stretch;
  display: flex;
  gap: 36px;
  height: 50px;
  padding-left: 16px;
}

.new-coins-primary-tabs button {
  background: transparent;
  border: 0;
  color: var(--new-coin-tab-muted);
  font-size: 17px;
  font-weight: 550;
  min-height: 44px;
  padding: 0;
  position: relative;
}

.new-coins-primary-tabs button[aria-pressed='true'] {
  color: var(--new-coin-ink);
  font-weight: 700;
}

.new-coins-primary-tabs button[aria-pressed='true']::after {
  background: var(--new-coin-signal);
  border-radius: 2px;
  bottom: 0;
  content: '';
  height: 3px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: min(72px, 100%);
}

.new-coins-lifecycle-filters {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  height: 36px;
  padding: 3px 16px;
}

.new-coins-lifecycle-filters button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--new-coin-muted);
  display: flex;
  font-size: 12px;
  height: 44px;
  justify-content: center;
  min-width: 0;
  padding: 7px 0;
}

.new-coins-lifecycle-filters button span {
  align-items: center;
  border-radius: 9px;
  display: flex;
  height: 30px;
  justify-content: center;
  overflow: hidden;
  padding: 0 5px;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.new-coins-lifecycle-filters button[aria-pressed='true'] span {
  background: var(--new-coin-filter-selected);
  color: var(--new-coin-ink);
  font-weight: 700;
}

.new-coins-project-content {
  padding: 8px 16px 18px;
}

.new-coins-project-content > h2 {
  align-items: center;
  display: flex;
  gap: 4px;
  font-size: 18px;
  font-weight: 700;
  height: 36px;
  line-height: 26px;
  margin: 0 0 12px;
}

.new-coins-project-list,
.new-coins-opportunity-list {
  display: grid;
  gap: 12px;
  margin: 0 auto;
  width: 100%;
}

.new-coins-opportunity-content {
  display: grid;
  gap: 10px;
  padding: 12px 16px 18px;
}

.new-coins-opportunity-filters {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 36px;
  margin: 0 auto;
  max-width: 358px;
  width: 100%;
}

.new-coins-opportunity-filters button {
  background: transparent;
  border: 0;
  color: var(--new-coin-muted);
  font-size: 11px;
  font-weight: 600;
  height: 44px;
  margin-top: -4px;
  min-width: 0;
  padding: 0;
}

.new-coins-opportunity-filters button span {
  align-items: center;
  background: var(--new-coin-opportunity-filter);
  border-radius: 12px;
  display: flex;
  height: 36px;
  justify-content: center;
  overflow: hidden;
  padding: 0 4px;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.new-coins-opportunity-filters button[aria-pressed='true'] span {
  background: var(--new-coin-action);
  color: var(--new-coin-action-ink);
}

.new-coins-inline-warning {
  align-items: center;
  color: var(--negative);
  display: flex;
  font-size: 11px;
  gap: 6px;
  margin: 0 auto 8px;
  max-width: 358px;
  min-height: 28px;
}

.new-coins-inline-warning span {
  flex: 1;
}

.new-coins-inline-warning button {
  background: transparent;
  border: 0;
  color: inherit;
  height: 44px;
  margin-block: -8px;
  width: 44px;
}

.new-coins-state {
  align-items: center;
  color: var(--new-coin-muted);
  display: flex;
  flex-direction: column;
  gap: 10px;
  justify-content: center;
  min-height: 188px;
  text-align: center;
}

.new-coins-state button {
  background: var(--new-coin-action);
  border: 0;
  border-radius: 12px;
  color: var(--new-coin-action-ink);
  min-height: 44px;
  padding: 0 20px;
}

@media (max-width: 340px) {
  .new-coins-primary-tabs {
    gap: 20px;
  }

  .new-coins-lifecycle-filters {
    gap: 2px;
    padding-inline: 10px;
  }

  .new-coins-lifecycle-filters button {
    font-size: 10px;
  }

  .new-coins-banner,
  .new-coins-project-content,
  .new-coins-opportunity-content {
    padding-inline: 16px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .new-coins-pencil {
    scroll-behavior: auto;
  }
}
</style>
