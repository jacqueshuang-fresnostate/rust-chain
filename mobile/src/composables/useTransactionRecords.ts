import { computed, ref } from 'vue'
import { fetchMarketPairs } from '@/api/market'
import {
  fetchMarginPositionExecutions,
  fetchMarginPositionRisk,
  fetchMarginPositions,
  fetchMarginProducts,
  fetchMarginWallets,
  fetchOpenSpotOrders,
  fetchSpotOrderHistory,
  type MarginPosition,
  type MarginPositionExecution,
  type MarginPositionRisk,
  type MarginCrossAccount,
  type MarginWalletAccount,
  type SpotOrder,
} from '@/api/trading'
import { isFilledMarginPosition, isPendingMarginPosition } from '@/core/marginOrder'
import { createOrdersRequestLifecycle, type OrdersRequestSnapshot } from '@/core/ordersRequest'
import type { TransactionRecordTab } from '@/core/transactionRecords'
import type { MarginProduct, MarketPair } from '@/core/types'
import { useSessionStore } from '@/stores/session'

interface TransactionRecordsPayload {
  currentSpot?: SpotOrder[]
  historySpot?: SpotOrder[]
  currentMargin?: MarginPosition[]
  historyMargin?: MarginPosition[]
  wallets?: MarginWalletAccount[]
  crossAccounts?: MarginCrossAccount[]
  products?: MarginProduct[]
  pairs?: MarketPair[]
  risks?: Map<string, MarginPositionRisk>
  executions?: Map<string, MarginPositionExecution[]>
}

export function useTransactionRecords() {
  const session = useSessionStore()
  const lifecycle = createOrdersRequestLifecycle()
  const currentSpot = ref<SpotOrder[]>([])
  const historySpot = ref<SpotOrder[]>([])
  const currentMargin = ref<MarginPosition[]>([])
  const historyMargin = ref<MarginPosition[]>([])
  const wallets = ref<MarginWalletAccount[]>([])
  const crossAccounts = ref<MarginCrossAccount[]>([])
  const products = ref<MarginProduct[]>([])
  const pairs = ref<MarketPair[]>([])
  const risks = ref(new Map<string, MarginPositionRisk>())
  const executions = ref(new Map<string, MarginPositionExecution[]>())
  const loading = ref(false)
  const error = ref<unknown | null>(null)

  const pendingMargin = computed(() => currentMargin.value.filter(isPendingMarginPosition))
  const openPositions = computed(() => currentMargin.value.filter(isFilledMarginPosition))

  function clear(): void {
    currentSpot.value = []
    historySpot.value = []
    currentMargin.value = []
    historyMargin.value = []
    wallets.value = []
    crossAccounts.value = []
    products.value = []
    pairs.value = []
    risks.value = new Map()
    executions.value = new Map()
    loading.value = false
    error.value = null
  }

  async function load(tab: TransactionRecordTab): Promise<void> {
    if (!session.isAuthenticated || tab === 'ledger' || tab.includes('strategy')) {
      lifecycle.invalidate()
      loading.value = false
      error.value = null
      return
    }
    loading.value = true
    error.value = null
    const snapshot: OrdersRequestSnapshot = {
      sessionGeneration: session.generation,
      market: tab === 'current' || tab === 'history' ? 'spot' : 'margin',
      state: tab === 'current' ? 'current' : tab === 'positions' ? 'positions' : 'history',
      workspace: tab,
    }
    const result = await lifecycle.load(snapshot, (signal) => fetchTab(tab, signal))
    if (result.state === 'stale') return
    loading.value = false
    if (result.state === 'error') {
      error.value = result.error
      return
    }
    commit(result.value)
  }

  async function fetchTab(tab: TransactionRecordTab, signal: AbortSignal): Promise<TransactionRecordsPayload> {
    if (tab === 'current') {
      const [spot, margin, nextProducts, nextPairs] = await Promise.all([
        fetchOpenSpotOrders(30, signal),
        fetchMarginPositions('opened', 30, signal),
        fetchMarginProducts(),
        fetchMarketPairs(),
      ])
      return { currentSpot: spot, currentMargin: margin, products: nextProducts, pairs: nextPairs }
    }
    if (tab === 'history') {
      const [spot, margin, nextProducts, nextPairs] = await Promise.all([
        fetchSpotOrderHistory(30, signal),
        fetchHistoryOrderPositions(signal),
        fetchMarginProducts(),
        fetchMarketPairs(),
      ])
      const nextExecutions = await fetchExecutions(margin, signal)
      return { historySpot: spot, historyMargin: margin, products: nextProducts, pairs: nextPairs, executions: nextExecutions }
    }
    if (tab === 'positions') {
      const [walletSnapshot, nextProducts, nextPairs] = await Promise.all([
        fetchMarginWallets(signal),
        fetchMarginProducts(),
        fetchMarketPairs(),
      ])
      const opened = walletSnapshot.positions.filter((position) => position.status.toLowerCase() === 'opened')
      const nextRisks = await fetchRisks(opened.filter(isFilledMarginPosition), signal)
      return {
        currentMargin: opened,
        wallets: walletSnapshot.wallets,
        crossAccounts: walletSnapshot.crossAccounts,
        products: nextProducts,
        pairs: nextPairs,
        risks: nextRisks,
      }
    }
    const [margin, nextProducts, nextPairs] = await Promise.all([
      fetchHistoryOrderPositions(signal),
      fetchMarginProducts(),
      fetchMarketPairs(),
    ])
    const nextExecutions = await fetchExecutions(margin, signal)
    return { historyMargin: margin, products: nextProducts, pairs: nextPairs, executions: nextExecutions }
  }

  async function fetchHistoryOrderPositions(signal: AbortSignal): Promise<MarginPosition[]> {
    const pages = await Promise.all([
      fetchMarginPositions('closed', 30, signal),
      fetchMarginPositions('liquidated', 30, signal),
      fetchMarginPositions('canceled', 30, signal),
    ])
    const historyOrderStatuses = new Set(['closed', 'liquidated', 'canceled', 'cancelled'])
    return [...new Map(pages.flat().map((position) => [position.id, position])).values()]
      .filter((position) => historyOrderStatuses.has(position.status.trim().toLowerCase()))
  }

  async function fetchRisks(
    positions: readonly MarginPosition[],
    signal: AbortSignal,
  ): Promise<Map<string, MarginPositionRisk>> {
    const settled = await Promise.allSettled(
      positions.map((position) => fetchMarginPositionRisk(position.id, signal)),
    )
    return new Map(settled.flatMap((result, index) => result.status === 'fulfilled'
      ? [[positions[index]!.id, result.value] as const]
      : []))
  }

  async function fetchExecutions(
    positions: readonly MarginPosition[],
    signal: AbortSignal,
  ): Promise<Map<string, MarginPositionExecution[]>> {
    const settled = await Promise.allSettled(
      positions.map((position) => fetchMarginPositionExecutions(position.id, signal)),
    )
    return new Map(settled.flatMap((result, index) => result.status === 'fulfilled'
      ? [[positions[index]!.id, result.value] as const]
      : []))
  }

  function commit(payload: TransactionRecordsPayload): void {
    if (payload.currentSpot) currentSpot.value = payload.currentSpot
    if (payload.historySpot) historySpot.value = payload.historySpot
    if (payload.currentMargin) currentMargin.value = payload.currentMargin
    if (payload.historyMargin) historyMargin.value = payload.historyMargin
    if (payload.wallets) wallets.value = payload.wallets
    if (payload.crossAccounts) crossAccounts.value = payload.crossAccounts
    if (payload.products) products.value = payload.products
    if (payload.pairs) pairs.value = payload.pairs
    if (payload.risks) risks.value = payload.risks
    if (payload.executions) executions.value = payload.executions
  }

  return {
    currentSpot,
    historySpot,
    currentMargin,
    historyMargin,
    wallets,
    crossAccounts,
    products,
    pairs,
    risks,
    executions,
    pendingMargin,
    openPositions,
    loading,
    error,
    load,
    clear,
    invalidate: lifecycle.invalidate,
    stop: lifecycle.stop,
  }
}
