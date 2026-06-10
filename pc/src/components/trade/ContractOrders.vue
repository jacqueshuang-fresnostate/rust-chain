<template>
  <div class="flex flex-col h-full">
    <!-- Tabs -->
    <div class="flex border-b border-border">
      <button
        v-for="t in tabs"
        :key="t.key"
        :class="['px-4 py-2 text-sm font-medium hover:text-primary transition-colors', activeTab === t.key ? 'text-primary border-b-2 border-primary' : 'text-muted-foreground']"
        @click="activeTab = t.key"
      >
        {{ t.label }}
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-auto p-2">
      <div v-if="loading" class="h-full flex items-center justify-center text-muted-foreground">
        <span class="animate-spin mr-2">⏳</span> {{ $t('common.loading') }}
      </div>
      <div v-else-if="isEmpty" class="h-full flex flex-col items-center justify-center text-muted-foreground opacity-50">
        <span class="text-4xl mb-2">📄</span>
        <span>{{ $t('trade.no_data') }}</span>
      </div>

      <!-- Positions (当前持仓) -->
      <div v-else-if="activeTab === 'positions'">
        <!-- Close All Button -->
        <div v-if="positionList.length > 0" class="flex justify-end mb-2">
          <button @click="handleCloseAll"
            :disabled="closingAll"
            class="text-xs px-3 py-1.5 rounded border border-destructive/30 text-destructive hover:bg-destructive/10 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1">
            <span v-if="closingAll" class="animate-spin">⏳</span>
            一键平仓
          </button>
        </div>
      <table class="w-full text-xs text-left">
        <thead>
          <tr class="text-muted-foreground border-b border-border">
            <th class="pb-2">合约</th>
            <th class="pb-2 text-right">开仓均价</th>
            <th class="pb-2 text-right">当前价格</th>
            <th class="pb-2 text-right">持仓量</th>
            <th class="pb-2 text-right">保证金</th>
            <th class="pb-2 text-right">保证金率</th>
            <th class="pb-2 text-right">未实现盈亏</th>
            <th class="pb-2 text-right">收益率</th>
            <th class="pb-2 text-right">{{ $t('trade.action') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="pos in positionList" :key="pos.key" class="border-b border-border/50 hover:bg-muted/50">
            <td class="py-2.5">
              <div class="flex items-center gap-1.5">
                <span class="font-bold font-mono">{{ pos.symbol.split('/')[0] }}</span>
                <span class="text-[10px] font-bold px-1 py-0.5 rounded leading-none"
                  :class="pos.direction === 'LONG' ? 'bg-up/10 text-up' : 'bg-down/10 text-down'">
                  {{ pos.direction === 'LONG' ? '多' : '空' }}
                </span>
                <span class="text-[10px] text-muted-foreground font-mono">{{ pos.leverage }}x</span>
              </div>
            </td>
            <td class="py-2.5 text-right font-mono">{{ formatPrice(pos.entryPrice) }}</td>
            <td class="py-2.5 text-right font-mono" :class="pos.currentPrice > pos.entryPrice ? 'text-up' : 'text-down'">
              {{ formatPrice(pos.currentPrice) }}
            </td>
            <td class="py-2.5 text-right font-mono">{{ pos.positionAmount }}</td>
            <td class="py-2.5 text-right font-mono">{{ numeral(pos.margin).format('0,0.00') }}</td>
            <td class="py-2.5 text-right font-mono"
              :class="pos.marginRate < 0.1 ? 'text-down' : pos.marginRate < 0.5 ? 'text-orange-400' : 'text-up'">
              {{ numeral(pos.marginRate).format('0.00%') }}
            </td>
            <td class="py-2.5 text-right font-mono font-bold"
              :class="pos.unrealizedPnl >= 0 ? 'text-up' : 'text-down'">
              {{ pos.unrealizedPnl >= 0 ? '+' : '' }}{{ numeral(pos.unrealizedPnl).format('0,0.00') }}
            </td>
            <td class="py-2.5 text-right font-mono font-bold"
              :class="pos.plRatio >= 0 ? 'text-up' : 'text-down'">
              {{ pos.plRatio >= 0 ? '+' : '' }}{{ numeral(pos.plRatio).format('0.00%') }}
            </td>
            <td class="py-2.5 text-right">
              <button @click="openCloseModal(pos)" class="text-xs text-primary hover:underline mr-2">{{ $t('trade.close_position') }}</button>
            </td>
          </tr>
        </tbody>
      </table>
      </div>

      <!-- Open Orders (当前委托) -->
      <table v-else-if="activeTab === 'open_orders'" class="w-full text-xs text-left">
        <thead>
          <tr class="text-muted-foreground border-b border-border">
            <th class="pb-2">Symbol</th>
            <th class="pb-2">Side</th>
            <th class="pb-2">Leverage</th>
            <th class="pb-2">Price</th>
            <th class="pb-2">Amount</th>
            <th class="pb-2">Status</th>
            <th class="pb-2 text-right">{{ $t('trade.action') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="item in currentOrders" :key="item.orderId" class="border-b border-border/50 hover:bg-muted/50">
            <td class="py-2 font-medium">{{ item.symbol }}</td>
            <td class="py-2" :class="getDirectionClass(item.direction)">{{ getDirectionText(item.direction) }}</td>
            <td class="py-2 font-mono">{{ item.leverage }}x</td>
            <td class="py-2 font-mono">{{ formatPrice(item.price) }}</td>
            <td class="py-2 font-mono">{{ item.amount }}</td>
            <td class="py-2">{{ formatStatus(item.status) }}</td>
            <td class="py-2 text-right">
                <button @click="openCancelModal(item)" class="text-destructive hover:underline text-xs">{{ $t('trade.cancel') }}</button>
            </td>
          </tr>
        </tbody>
      </table>

      <!-- History (历史委托) -->
      <table v-else-if="activeTab === 'history'" class="w-full text-xs text-left">
        <thead>
          <tr class="text-muted-foreground border-b border-border">
            <th class="pb-2">Time</th>
            <th class="pb-2">Symbol</th>
            <th class="pb-2">Side</th>
            <th class="pb-2">Type</th>
            <th class="pb-2">Price</th>
            <th class="pb-2">Amount</th>
            <th class="pb-2">Status</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="item in historyOrders" :key="item.orderId" class="border-b border-border/50 hover:bg-muted/50">
            <td class="py-2">{{ formatTime(item.createTime) }}</td>
            <td class="py-2 font-medium">{{ item.symbol }}</td>
            <td class="py-2" :class="getDirectionClass(item.direction)">{{ getDirectionText(item.direction) }}</td>
            <td class="py-2">{{ item.type === 0 ? $t('trade.limit') : $t('trade.market') }}</td>
            <td class="py-2 font-mono">{{ formatPrice(item.price) }}</td>
            <td class="py-2 font-mono">{{ item.amount }}</td>
            <td class="py-2">{{ formatStatus(item.status) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Close Position Modal -->
    <div v-if="showCloseModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showCloseModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-96 shadow-xl">
        <!-- Header -->
        <div class="flex items-center justify-between mb-4">
          <div class="text-base font-bold">平仓</div>
          <button @click="showCloseModal = false" class="text-muted-foreground hover:text-foreground transition-colors text-lg leading-none">&times;</button>
        </div>

        <!-- Position Info -->
        <div v-if="closingPosition" class="flex items-center gap-2 mb-4 p-2.5 bg-muted/30 rounded-lg border border-border/50">
          <span class="font-bold font-mono text-sm">{{ closingPosition.symbol.split('/')[0] }}</span>
          <span class="text-[10px] font-bold px-1.5 py-0.5 rounded leading-none"
            :class="closingPosition.direction === 'LONG' ? 'bg-up/10 text-up' : 'bg-down/10 text-down'">
            {{ closingPosition.direction === 'LONG' ? '多' : '空' }}
          </span>
          <span class="text-[10px] text-muted-foreground font-mono">{{ closingPosition.leverage }}x</span>
          <span class="ml-auto text-xs text-muted-foreground">
            可平: <span class="font-mono font-bold text-foreground">{{ closingPosition.avaPosition }}</span> 张
          </span>
        </div>

        <!-- Order Type Toggle -->
        <div class="flex gap-2 mb-4">
          <span @click="closeOrderType = 0"
            :class="closeOrderType === 0 ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'"
            class="text-xs cursor-pointer transition-colors">
            {{ $t('trade.market') }}
          </span>
          <span @click="closeOrderType = 1"
            :class="closeOrderType === 1 ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'"
            class="text-xs cursor-pointer transition-colors">
            {{ $t('trade.limit') }}
          </span>
        </div>

        <!-- Price Input (Limit only) -->
        <div v-if="closeOrderType === 1" class="mb-3">
          <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
            <span class="text-xs text-muted-foreground w-12 shrink-0">{{ $t('trade.price') }}</span>
            <input
              v-model="closePrice"
              type="number"
              class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
              placeholder="0.00"
            />
            <span class="text-xs text-muted-foreground ml-2 w-10 text-right">USDT</span>
          </div>
        </div>
        <div v-else class="h-10 flex items-center px-3 bg-muted/20 border border-transparent rounded text-sm text-muted-foreground mb-3">
          {{ $t('trade.market_price') }}
        </div>

        <!-- Amount Input -->
        <div class="mb-2">
          <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
            <span class="text-xs text-muted-foreground w-12 shrink-0">数量</span>
            <input
              v-model="closeVolume"
              type="number"
              class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
              placeholder="0"
              :max="closingPosition?.avaPosition"
              min="0"
            />
            <span class="text-xs text-muted-foreground ml-2 w-10 text-right">张</span>
          </div>
        </div>

        <!-- Percent Shortcuts -->
        <div class="flex gap-2 mb-4">
          <button v-for="p in [25, 50, 75, 100]" :key="p"
            @click="setClosePercent(p)"
            class="flex-1 bg-muted/50 hover:bg-muted text-[10px] py-1.5 rounded border border-transparent hover:border-border transition-all font-medium">
            {{ p }}%
          </button>
        </div>

        <!-- Action Buttons -->
        <div class="flex gap-3">
          <button @click="showCloseModal = false"
            class="flex-1 py-2.5 text-sm border border-border rounded-lg hover:bg-muted transition-colors font-medium">
            {{ $t('common.cancel') }}
          </button>
          <button @click="confirmClosePosition"
            :disabled="closing || !canClose"
            class="flex-1 py-2.5 text-sm rounded-lg font-bold text-white transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center"
            :class="closingPosition?.direction === 'LONG' ? 'bg-down hover:bg-down/90' : 'bg-up hover:bg-up/90'">
            <span v-if="closing" class="animate-spin mr-1">⏳</span>
            确认平仓
          </button>
        </div>
      </div>
    </div>

    <!-- Close All Confirm Modal -->
    <div v-if="showCloseAllModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showCloseAllModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80 shadow-xl">
        <div class="flex items-center justify-between mb-4">
          <div class="text-base font-bold">一键平仓</div>
          <button @click="showCloseAllModal = false" class="text-muted-foreground hover:text-foreground transition-colors text-lg leading-none">&times;</button>
        </div>
        <div class="text-sm text-muted-foreground mb-2">
          确认一键平仓所有持仓？
        </div>
        <div class="text-xs text-destructive/80 mb-5 p-2 bg-destructive/5 rounded border border-destructive/10">
          ⚠️ 此操作将以市价平掉所有多仓和空仓，不可撤销。
        </div>
        <div class="flex gap-3">
          <button @click="showCloseAllModal = false"
            class="flex-1 py-2.5 text-sm border border-border rounded-lg hover:bg-muted transition-colors font-medium">
            {{ $t('common.cancel') }}
          </button>
          <button @click="confirmCloseAll"
            :disabled="closingAll"
            class="flex-1 py-2.5 text-sm rounded-lg font-bold text-white bg-destructive hover:bg-destructive/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center">
            <span v-if="closingAll" class="animate-spin mr-1">⏳</span>
            确认平仓
          </button>
        </div>
      </div>
    </div>

    <!-- Cancel Order Confirm Modal -->
    <div v-if="showCancelModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showCancelModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80 shadow-xl">
        <div class="flex items-center justify-between mb-4">
          <div class="text-base font-bold">撤销委托</div>
          <button @click="showCancelModal = false" class="text-muted-foreground hover:text-foreground transition-colors text-lg leading-none">&times;</button>
        </div>
        <div v-if="cancelingOrder" class="text-sm text-muted-foreground mb-4 space-y-1">
          <div class="flex justify-between"><span>交易对</span><span class="font-mono font-medium text-foreground">{{ cancelingOrder.symbol }}</span></div>
          <div class="flex justify-between"><span>方向</span><span :class="getDirectionClass(cancelingOrder.direction)" class="font-medium">{{ getDirectionText(cancelingOrder.direction) }}</span></div>
          <div class="flex justify-between"><span>价格</span><span class="font-mono text-foreground">{{ formatPrice(cancelingOrder.price) }}</span></div>
          <div class="flex justify-between"><span>数量</span><span class="font-mono text-foreground">{{ cancelingOrder.amount }}</span></div>
        </div>
        <div class="text-xs text-muted-foreground mb-5">确认撤销该委托订单？</div>
        <div class="flex gap-3">
          <button @click="showCancelModal = false"
            class="flex-1 py-2.5 text-sm border border-border rounded-lg hover:bg-muted transition-colors font-medium">
            {{ $t('common.cancel') }}
          </button>
          <button @click="confirmCancel"
            :disabled="canceling"
            class="flex-1 py-2.5 text-sm rounded-lg font-bold text-white bg-destructive hover:bg-destructive/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center">
            <span v-if="canceling" class="animate-spin mr-1">⏳</span>
            确认撤单
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useContractStore, type ContractOrder } from '@/stores/contract'
import { useToast } from 'vue-toastification'
import numeral from 'numeral'
import type { OrderType } from '@/api/contract'

const props = defineProps<{
  symbol?: string
}>()

const { t: $t } = useI18n()
const toast = useToast()
const contractStore = useContractStore()

const tabs = [
    { key: 'positions', label: '当前持仓' },
    { key: 'open_orders', label: '当前委托' },
    { key: 'history', label: '历史委托' }
]
const activeTab = ref('positions')
const loading = ref(false)

// Close position modal state
const showCloseModal = ref(false)
const closingPosition = ref<PositionItem | null>(null)
const closeOrderType = ref<OrderType>(0) // 0=市价 1=限价
const closePrice = ref<number | null>(null)
const closeVolume = ref<number | null>(null)
const closing = ref(false)
const closingAll = ref(false)
const showCloseAllModal = ref(false)
const showCancelModal = ref(false)
const cancelingOrder = ref<ContractOrder | null>(null)
const canceling = ref(false)

const currentOrders = computed(() => contractStore.currentOrders)
const historyOrders = computed(() => contractStore.historyOrders)
const activeCoinId = computed(() => contractStore.activeCoin?.id)

const canClose = computed(() => {
    if (!closeVolume.value || closeVolume.value <= 0) return false
    if (!closingPosition.value) return false
    if (closeVolume.value > closingPosition.value.avaPosition) return false
    if (closeOrderType.value === 1 && (!closePrice.value || closePrice.value <= 0)) return false
    return true
})

// Build position list from wallets — only items with actual positions (buy or sell > 0)
interface PositionItem {
    key: string
    symbol: string
    direction: 'LONG' | 'SHORT'
    positionAmount: number
    avaPosition: number
    entryPrice: number
    currentPrice: number
    leverage: number
    margin: number
    marginRate: number
    unrealizedPnl: number
    plRatio: number
    shareNumber: number
}

const positionList = computed<PositionItem[]>(() => {
    const list: PositionItem[] = []
    for (const w of contractStore.wallets) {
        // Prefer real-time thumb price from WS, fallback to wallet's static price
        const curPrice = contractStore.getThumbBySymbol(w.symbol)?.last || w.currentPrice || 0
        if (curPrice <= 0) continue

        let buyPl = 0, buyMinNeedPrinc = 0, buyCloseFee = 0
        let sellPl = 0, sellMinNeedPrinc = 0, sellCloseFee = 0

        // Long position
        if (w.usdtBuyPosition > 0 || w.usdtFrozenBuyPosition > 0) {
            const totalPos = w.usdtBuyPosition + w.usdtFrozenBuyPosition
            buyPl = (curPrice / w.usdtBuyPrice - 1) * totalPos * w.usdtShareNumber
            buyMinNeedPrinc = totalPos * w.usdtShareNumber / w.usdtBuyLeverage
            buyCloseFee = totalPos * w.usdtShareNumber * w.closeFee

            let mRate = 0
            if (w.usdtPattern === 'FIXED') {
                mRate = buyMinNeedPrinc > 0 ? (buyPl + w.usdtBuyPrincipalAmount - buyCloseFee) / buyMinNeedPrinc : 0
            }

            list.push({
                key: `${w.id}_long`,
                symbol: w.symbol,
                direction: 'LONG',
                positionAmount: totalPos,
                avaPosition: w.usdtBuyPosition,
                entryPrice: w.usdtBuyPrice,
                currentPrice: curPrice,
                leverage: w.usdtBuyLeverage,
                margin: w.usdtBuyPrincipalAmount,
                marginRate: mRate,
                unrealizedPnl: buyPl,
                plRatio: w.usdtBuyPrincipalAmount > 0 ? buyPl / w.usdtBuyPrincipalAmount : 0,
                shareNumber: w.usdtShareNumber
            })
        }
        // Short position
        if (w.usdtSellPosition > 0 || w.usdtFrozenSellPosition > 0) {
            const totalPos = w.usdtSellPosition + w.usdtFrozenSellPosition
            sellPl = (1 - curPrice / w.usdtSellPrice) * totalPos * w.usdtShareNumber
            sellMinNeedPrinc = totalPos * w.usdtShareNumber / w.usdtSellLeverage
            sellCloseFee = totalPos * w.usdtShareNumber * w.closeFee

            let mRate = 0
            if (w.usdtPattern === 'FIXED') {
                mRate = sellMinNeedPrinc > 0 ? (sellPl + w.usdtSellPrincipalAmount - sellCloseFee) / sellMinNeedPrinc : 0
            }

            list.push({
                key: `${w.id}_short`,
                symbol: w.symbol,
                direction: 'SHORT',
                positionAmount: totalPos,
                avaPosition: w.usdtSellPosition,
                entryPrice: w.usdtSellPrice,
                currentPrice: curPrice,
                leverage: w.usdtSellLeverage,
                margin: w.usdtSellPrincipalAmount,
                marginRate: mRate,
                unrealizedPnl: sellPl,
                plRatio: w.usdtSellPrincipalAmount > 0 ? sellPl / w.usdtSellPrincipalAmount : 0,
                shareNumber: w.usdtShareNumber
            })
        }

        // CROSSED mode: override marginRate for all positions under this wallet
        if (w.usdtPattern === 'CROSSED') {
            const crossedRate = (buyMinNeedPrinc + sellMinNeedPrinc) > 0
                ? (buyPl + sellPl + w.usdtBuyPrincipalAmount + w.usdtSellPrincipalAmount
                   + w.usdtBalance + w.usdtFrozenBalance - buyCloseFee - sellCloseFee)
                  / (buyMinNeedPrinc + sellMinNeedPrinc)
                : 0
            // Update marginRate for positions just added
            for (const pos of list) {
                if (pos.key.startsWith(`${w.id}_`)) {
                    pos.marginRate = crossedRate
                }
            }
        }
    }
    return list
})

const isEmpty = computed(() => {
    if (activeTab.value === 'positions') return positionList.value.length === 0
    if (activeTab.value === 'open_orders') return currentOrders.value.length === 0
    return historyOrders.value.length === 0
})

const formatPrice = (val: number | undefined | null) => {
    if (val === undefined || val === null || val === 0) return '--'
    return numeral(val).format(val < 1 ? '0.000000' : '0,0.00')
}

const formatTime = (ts: number | undefined) => {
    if (!ts) return '--'
    return new Date(ts).toLocaleString()
}

const formatStatus = (status: number | string) => {
    const map: Record<number | string, string> = {
        0: 'Pending',
        1: 'Completed',
        2: 'Canceled',
        3: 'Failed',
        'TRADING': 'Pending',
        'COMPLETED': 'Completed',
        'CANCELED': 'Canceled',
        'FAILED': 'Failed'
    }
    return map[status] ?? String(status)
}

const getDirectionText = (direction: number) => {
    const map: Record<number, string> = {
        0: $t('trade.open_long'),
        1: $t('trade.open_short'),
        2: $t('trade.close_long'),
        3: $t('trade.close_short')
    }
    return map[direction] ?? String(direction)
}

const getDirectionClass = (direction: number) => {
    return direction === 0 || direction === 2 ? 'text-up' : 'text-down'
}

const loadData = async () => {
    loading.value = true
    try {
        if (activeTab.value === 'positions') {
            await contractStore.loadWallets()
        } else if (activeTab.value === 'open_orders') {
            await contractStore.loadCurrentOrders(activeCoinId.value)
        } else {
            await contractStore.loadHistoryOrders(activeCoinId.value)
        }
    } finally {
        loading.value = false
    }
}

/** Open cancel order confirmation modal */
const openCancelModal = (order: ContractOrder) => {
    cancelingOrder.value = order
    canceling.value = false
    showCancelModal.value = true
}

/** Confirm and cancel the order */
const confirmCancel = async () => {
    if (!cancelingOrder.value) return
    canceling.value = true
    try {
        await contractStore.cancel(cancelingOrder.value.orderId, activeCoinId.value)
        toast.success($t('trade.cancel_success'))
        showCancelModal.value = false
    } catch (e) {
        toast.error($t('trade.cancel_failed'))
    } finally {
        canceling.value = false
    }
}

/** Open close-all confirmation modal */
const handleCloseAll = () => {
    showCloseAllModal.value = true
}

/** Confirm and close all positions at market price */
const confirmCloseAll = async () => {
    const coinId = activeCoinId.value
    if (!coinId) return
    closingAll.value = true
    try {
        // type 2 = 市价平多+平空
        await contractStore.submitCloseAll(coinId, 2)
        toast.success($t('trade.close_success'))
        showCloseAllModal.value = false
    } catch (e: any) {
        toast.error(e?.response?.data?.message || $t('trade.close_failed'))
    } finally {
        closingAll.value = false
    }
}

/** Open the close-position modal and pre-fill values */
const openCloseModal = (pos: PositionItem) => {
    closingPosition.value = pos
    closeOrderType.value = 0 // default to market
    closePrice.value = pos.currentPrice || null
    closeVolume.value = pos.avaPosition // default to full available position
    closing.value = false
    showCloseModal.value = true
}

/** Set volume by percentage of available position */
const setClosePercent = (p: number) => {
    if (!closingPosition.value) return
    closeVolume.value = Math.floor(closingPosition.value.avaPosition * p / 100)
}

/** Confirm and submit the close position order */
const confirmClosePosition = async () => {
    const pos = closingPosition.value
    const coinId = activeCoinId.value
    if (!pos || !coinId) return
    if (!closeVolume.value || closeVolume.value <= 0) return

    closing.value = true
    try {
        // 平仓: direction 0=买入平空(close SHORT), 1=卖出平多(close LONG)
        const direction = pos.direction === 'LONG' ? 1 : 0
        await contractStore.submitClosePosition({
            contractCoinId: coinId,
            direction: direction as 0 | 1,
            type: closeOrderType.value as 0 | 1,
            triggerPrice: 0,
            entrustPrice: closeOrderType.value === 1 ? (closePrice.value || 0) : 0,
            volume: closeVolume.value
        })
        toast.success($t('trade.close_success'))
        showCloseModal.value = false
    } catch (e: any) {
        toast.error(e?.response?.data?.message || $t('trade.close_failed'))
    } finally {
        closing.value = false
    }
}

watch([() => props.symbol, activeTab, activeCoinId], () => {
    loadData()
})

watch(() => contractStore.orderRefreshKey, () => {
    loadData()
})

onMounted(() => {
    loadData()
})
</script>