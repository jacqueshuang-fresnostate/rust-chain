<script setup lang="ts">
import { computed } from 'vue'
import { ChevronLeft } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { goBackOr } from '@/core/navigation'
import type { TransactionRecordTab } from '@/core/transactionRecords'

const props = withDefaults(defineProps<{
  activeTab: TransactionRecordTab
  backFallback?: RouteLocationRaw
}>(), {
  backFallback: () => ({ name: 'home' }),
})

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const labels: Record<TransactionRecordTab, string> = {
  current: 'orders.current',
  history: 'orders.historyOrdersTab',
  positions: 'orders.positionsAssetsTab',
  'position-history': 'ledger.positionHistoryTab',
  ledger: 'ledger.transactionLedgerTab',
  'current-strategy': 'ledger.currentStrategyTab',
  'strategy-history': 'ledger.strategyHistoryTab',
}

function tabTarget(tab: TransactionRecordTab): RouteLocationRaw {
  const symbol = typeof route.query.symbol === 'string' && route.query.symbol.trim()
    ? route.query.symbol
    : undefined
  const query = symbol ? { tab, symbol } : { tab }
  if (tab === 'ledger') return { name: 'wallet-ledger', query: symbol ? { symbol } : undefined }
  return { name: 'orders', query }
}

async function back(): Promise<void> {
  await goBackOr(router, route.meta.backFallback || props.backFallback)
}

const visibleTabs = computed<TransactionRecordTab[]>(() => {
  if (props.activeTab === 'position-history') {
    return ['current', 'positions', 'position-history', 'ledger']
  }
  if (props.activeTab === 'ledger' || props.activeTab.includes('strategy')) {
    return ['position-history', 'ledger', 'current-strategy', 'strategy-history']
  }
  return ['current', 'history', 'positions', 'position-history']
})
</script>

<template>
  <main
    class="page page--plain pencil-page records-workspace"
    data-transaction-records-workspace="pencil"
  >
    <header class="records-header">
      <button class="records-header__back" type="button" :aria-label="t('common.back')" @click="back">
        <ChevronLeft :size="26" aria-hidden="true" />
      </button>
      <h1>{{ t('ledger.title') }}</h1>
      <span class="records-header__placeholder" aria-hidden="true" />
    </header>

    <nav class="records-tabs" :class="{ 'records-tabs--ledger-window': activeTab === 'ledger' || activeTab.includes('strategy') }" :aria-label="t('ledger.recordTabsLabel')">
      <RouterLink
        v-for="tab in visibleTabs"
        :key="tab"
        class="records-tab"
        :class="{ 'is-active': activeTab === tab }"
        :to="tabTarget(tab)"
        :aria-current="activeTab === tab ? 'page' : undefined"
      >
        <span>{{ t(labels[tab]) }}</span>
        <i aria-hidden="true" />
      </RouterLink>
    </nav>

    <slot />
  </main>
</template>

<style scoped>
.records-workspace {
  --records-active: #18d38d;
  --records-button: #f3f6f4;
  --records-canvas: #ffffff;
  --records-chip: #f1f3f2;
  --records-chip-negative: #ffe8ed;
  --records-chip-positive: #ddf8eb;
  --records-divider: #edf1ef;
  --records-ink: #111714;
  --records-negative: #ff5878;
  --records-positive: #0dbe7b;
  --records-tab-line: #eef1ef;
  --records-tab-muted: #7b8680;
  --records-muted: #8a948f;
  background: var(--records-canvas);
  color: var(--records-ink);
  min-width: 0;
  overflow-x: clip;
}

:global(html[data-theme='dark'] .records-workspace) {
  --records-button: #111a15;
  --records-canvas: #000000;
  --records-chip: #151e19;
  --records-chip-negative: #32161f;
  --records-chip-positive: #103326;
  --records-divider: #17221c;
  --records-ink: #f3f7f5;
  --records-positive: #45efae;
  --records-tab-line: #18231d;
  --records-tab-muted: #8f9b94;
  --records-muted: #8f9b94;
}

.records-header {
  align-items: center;
  background: var(--records-canvas);
  box-sizing: border-box;
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr) 26px;
  height: 58px;
  min-height: 58px;
  padding: 0 16px;
  position: sticky;
  top: env(safe-area-inset-top);
  z-index: var(--layer-sticky-header);
}

.records-header::before {
  background: var(--records-canvas);
  bottom: 100%;
  content: '';
  height: env(safe-area-inset-top);
  inset-inline: 0;
  position: absolute;
}

.records-header h1 {
  color: var(--records-ink);
  font-size: 22px;
  font-weight: 700;
  line-height: 30px;
  margin: 0;
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.records-header__back {
  background: transparent;
  border: 0;
  color: var(--records-ink);
  display: grid;
  height: 26px;
  padding: 0;
  place-items: center;
  position: relative;
  width: 26px;
}

.records-header__back::before {
  content: '';
  height: 44px;
  left: 50%;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 44px;
}

.records-header__placeholder {
  height: 26px;
  width: 26px;
}

.records-tabs {
  background: var(--records-canvas);
  border-bottom: 1px solid var(--records-tab-line);
  box-sizing: border-box;
  column-gap: 2px;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 52px;
  min-height: 52px;
  overflow: hidden;
  padding: 0 8px;
}

.records-tabs--ledger-window {
  padding-inline: 10px;
}

.records-tab {
  color: var(--records-tab-muted);
  display: grid;
  font-size: 13px;
  font-weight: 500;
  gap: 9px;
  grid-template-rows: minmax(0, 1fr) 3px;
  height: 51px;
  line-height: 18px;
  min-width: 0;
  text-align: center;
  text-decoration: none;
}

.records-tab span {
  align-self: end;
  min-width: 0;
  overflow: hidden;
  padding-inline: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.records-tab i {
  background: transparent;
  display: block;
  height: 3px;
  width: 100%;
}

.records-tab.is-active {
  color: var(--records-ink);
  font-weight: 700;
}

.records-tab.is-active i {
  background: var(--records-active);
}

.records-header__back:focus-visible,
.records-tab:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus-ring);
  outline: 0;
}
</style>
