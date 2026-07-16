import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
    fetchContractSymbols,
    fetchSymbolThumb,
    fetchCurrentOrders,
    fetchHistoryOrders,
    fetchContractWallets,
    fetchWalletDetail,
    openPosition,
    closePosition,
    closeAllPositions,
    cancelOrder,
    cancelAllOrders,
    transferFunds,
    modifyLeverage,
    switchPattern,
    type OpenPositionParams,
    type ClosePositionParams,
    type TransferParams,
    type OrderListParams,
    type OrderType
} from '@/api/contract'

export interface ContractCoin {
    id: number
    symbol: string
    logoUrl: string
    baseSymbol: string
    coinSymbol: string
    enable: boolean
    sort: number
    minTurnover: number
    maxLeverage: number
    marginRate: number
    marginModes: Array<'cross' | 'isolated'>
    leverage: number[]
}

export interface CoinThumb {
    symbol: string
    open: number
    close: number
    last: number
    high: number
    low: number
    vol: number
    change: number
}

export interface ContractOrder {
    orderId: string
    symbol: string
    price: number
    amount: number
    direction: number
    type: OrderType
    leverage: number
    tradedAmount: number
    status: number
    createTime: number
}

export interface ContractWallet {
    id: number
    memberId: number
    symbol: string
    coinSymbol: string
    baseSymbol: string
    // Balances
    usdtBalance: number
    usdtFrozenBalance: number
    // Buy (Long) Position
    usdtBuyPosition: number
    usdtBuyPrice: number
    usdtBuyLeverage: number
    usdtBuyPrincipalAmount: number
    usdtFrozenBuyPosition: number
    // Sell (Short) Position
    usdtSellPosition: number
    usdtSellPrice: number
    usdtSellLeverage: number
    usdtSellPrincipalAmount: number
    usdtFrozenSellPosition: number
    // Share & P&L
    usdtShareNumber: number
    usdtPattern: string
    usdtTotalProfitAndLoss: number
    // Current price from contractCoin
    currentPrice: number
    // Fee
    closeFee: number
    maintenanceMarginRate: number
}

export const useContractStore = defineStore('contract', () => {
    const coins = ref<ContractCoin[]>([])
    const thumbs = ref<CoinThumb[]>([])
    const currentOrders = ref<ContractOrder[]>([])
    const historyOrders = ref<ContractOrder[]>([])
    const wallets = ref<ContractWallet[]>([])
    const activeCoin = ref<ContractCoin | null>(null)
    const loading = ref(false)

    const activeSymbol = computed(() => activeCoin.value?.symbol || '')

    async function loadCoins() {
        console.log('--- loadCoins ---')
        try {
            const res = await fetchContractSymbols()
            console.log('--- loadCoins raw response ---', res.data)
            if (res?.data) {
                coins.value = res.data.map((c: any) => ({
                    id: c.id,
                    symbol: c.symbol,
                    logoUrl: typeof c.logoUrl === 'string' ? c.logoUrl : '',
                    baseSymbol: c.baseSymbol,
                    coinSymbol: c.coinSymbol,
                    enable: c.enable,
                    sort: c.sort,
                    minTurnover: c.minTurnover,
                    maxLeverage: c.maxLeverage,
                    marginRate: c.marginRate,
                    marginModes: Array.isArray(c.marginModes) && c.marginModes.length > 0 ? c.marginModes : ['isolated'],
                    leverage: Array.isArray(c.leverage) ? c.leverage : (typeof c.leverage === 'string' ? c.leverage.split(',').map(Number) : [1, 2, 3, 5, 10, 20, 50, 100])
                }))
                console.log('--- coins ---', coins.value)
                if (!activeCoin.value && coins.value.length > 0) {
                    activeCoin.value = coins.value[0]
                }
            }
        } catch (e) {
            console.error('loadCoins error', e)
        }
    }

    async function loadThumbs() {
        try {
            const res = await fetchSymbolThumb()
            if (res.data?.data) {
                thumbs.value = res.data.data.map((t: any) => {
                    const open = Number(t.open) || 0
                    const close = Number(t.close ?? t.last) || 0
                    return {
                        symbol: t.symbol,
                        open,
                        close,
                        last: Number(t.last) || close,
                        high: Number(t.high) || 0,
                        low: Number(t.low) || 0,
                        vol: Number(t.vol) || 0,
                        change: open ? ((close - open) / open * 100) : 0
                    }
                })
            }
        } catch (e) {
            console.error('loadThumbs error', e)
        }
    }

    async function loadCurrentOrders(contractCoinId?: number) {
        try {
            const params: OrderListParams = {
                contractCoinId,
                pageNo: 1,
                pageSize: 50
            }
            const res = await fetchCurrentOrders(params)
            console.log('=== fetchCurrentOrders FULL res ===', JSON.stringify(res.data))

            // The response body (res.data) could be:
            // 1) { code: 0, data: [...] }         → array directly
            // 2) { code: 0, data: { content: [...] } }  → Spring Page
            // 3) { code: 0, data: { records: [...] } }  → MyBatis Plus Page
            // 4) [...] directly (no wrapper)
            const body = res.data
            let list: any[] = []

            if (Array.isArray(body)) {
                list = body
            } else if (body && typeof body === 'object') {
                const inner = body.data ?? body
                if (Array.isArray(inner)) {
                    list = inner
                } else if (inner && typeof inner === 'object') {
                    list = inner.content || inner.records || inner.list || []
                }
            }

            console.log('=== currentOrders extracted list ===', list.length, list)

            currentOrders.value = list.map((o: any) => ({
                orderId: o.orderId || o.id || o.entrustId || o.contractOrderEntrustId || '',
                symbol: o.symbol || o.coinSymbol || o.contractCoinName || '',
                price: Number(o.price || o.entrustPrice || o.currentPrice) || 0,
                amount: Number(o.amount || o.volume || o.shareNumber) || 0,
                direction: o.direction ?? o.entrustType ?? o.type ?? 0,
                type: o.type ?? o.entrustType ?? 0,
                leverage: Number(o.leverage || o.multiple) || 1,
                tradedAmount: Number(o.tradedAmount || o.tradedVolume || o.dealVolume) || 0,
                status: o.status ?? o.entrustStatus ?? 0,
                createTime: o.createTime || o.time || o.createDate || 0
            }))
            console.log('=== currentOrders.value ===', currentOrders.value)
        } catch (e) {
            console.error('loadCurrentOrders error', e)
        }
    }

    async function loadHistoryOrders(contractCoinId?: number) {
        try {
            const params: OrderListParams = {
                contractCoinId,
                pageNo: 1,
                pageSize: 50
            }
            const res = await fetchHistoryOrders(params)
            console.log('=== fetchHistoryOrders FULL res ===', JSON.stringify(res.data))

            const body = res.data
            let list: any[] = []

            if (Array.isArray(body)) {
                list = body
            } else if (body && typeof body === 'object') {
                const inner = body.data ?? body
                if (Array.isArray(inner)) {
                    list = inner
                } else if (inner && typeof inner === 'object') {
                    list = inner.content || inner.records || inner.list || []
                }
            }

            console.log('=== historyOrders extracted list ===', list.length, list)

            historyOrders.value = list.map((o: any) => ({
                orderId: o.orderId || o.id || o.entrustId || o.contractOrderEntrustId || '',
                symbol: o.symbol || o.coinSymbol || o.contractCoinName || '',
                price: Number(o.price || o.entrustPrice || o.currentPrice) || 0,
                amount: Number(o.amount || o.volume || o.shareNumber) || 0,
                direction: o.direction ?? o.entrustType ?? o.type ?? 0,
                type: o.type ?? o.entrustType ?? 0,
                leverage: Number(o.leverage || o.multiple) || 1,
                tradedAmount: Number(o.tradedAmount || o.tradedVolume || o.dealVolume) || 0,
                status: o.status ?? o.entrustStatus ?? 0,
                createTime: o.createTime || o.time || o.createDate || 0
            }))
            console.log('=== historyOrders.value ===', historyOrders.value)
        } catch (e) {
            console.error('loadHistoryOrders error', e)
        }
    }

    async function loadWallets() {
        try {
            const res = await fetchContractWallets()
            console.log('=== loadWallets raw ===', res.data)
            const body = res.data
            const list = body?.data || (Array.isArray(body) ? body : [])

            wallets.value = list.map((w: any) => ({
                id: w.id,
                memberId: w.memberId,
                symbol: w.contractCoin?.symbol || '',
                coinSymbol: w.contractCoin?.coinSymbol || '',
                baseSymbol: w.contractCoin?.baseSymbol || 'USDT',
                // Balances
                usdtBalance: Number(w.usdtBalance) || 0,
                usdtFrozenBalance: Number(w.usdtFrozenBalance) || 0,
                // Buy (Long)
                usdtBuyPosition: Number(w.usdtBuyPosition) || 0,
                usdtBuyPrice: Number(w.usdtBuyPrice) || 0,
                usdtBuyLeverage: Number(w.usdtBuyLeverage) || 1,
                usdtBuyPrincipalAmount: Number(w.usdtBuyPrincipalAmount) || 0,
                usdtFrozenBuyPosition: Number(w.usdtFrozenBuyPosition) || 0,
                // Sell (Short)
                usdtSellPosition: Number(w.usdtSellPosition) || 0,
                usdtSellPrice: Number(w.usdtSellPrice) || 0,
                usdtSellLeverage: Number(w.usdtSellLeverage) || 1,
                usdtSellPrincipalAmount: Number(w.usdtSellPrincipalAmount) || 0,
                usdtFrozenSellPosition: Number(w.usdtFrozenSellPosition) || 0,
                // Share & P&L
                usdtShareNumber: Number(w.usdtShareNumber) || 1,
                usdtPattern: w.usdtPattern || 'FIXED',
                usdtTotalProfitAndLoss: Number(w.usdtTotalProfitAndLoss) || 0,
                // Current price
                currentPrice: Number(w.contractCoin?.currentPrice || w.currentPrice) || 0,
                // Fees
                closeFee: Number(w.contractCoin?.closeFee) || 0.0001,
                maintenanceMarginRate: Number(w.contractCoin?.maintenanceMarginRate) || 0.005
            }))
            console.log('=== wallets parsed ===', wallets.value)
        } catch (e) {
            console.error('loadWallets error', e)
        }
    }

    async function getWalletDetail(contractCoinId: number) {
        const res = await fetchWalletDetail(contractCoinId)
        return res.data?.data
    }

    const orderRefreshKey = ref(0)

    function triggerOrderRefresh() {
        orderRefreshKey.value++
    }

    const delay = (ms: number) => new Promise(r => setTimeout(r, ms))

    async function submitOpenPosition(params: OpenPositionParams) {
        loading.value = true
        try {
            const res = await openPosition(params)
            await delay(500)
            await Promise.all([loadCurrentOrders(params.contractCoinId), loadWallets()])
            triggerOrderRefresh()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitClosePosition(params: ClosePositionParams) {
        loading.value = true
        try {
            const res = await closePosition(params)
            await delay(500)
            await Promise.all([loadCurrentOrders(params.contractCoinId), loadWallets()])
            triggerOrderRefresh()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitCloseAllPositions(contractCoinId?: number) {
        loading.value = true
        try {
            const res = await closeAllPositions(contractCoinId || 0, 0)
            await delay(500)
            await Promise.all([loadCurrentOrders(contractCoinId), loadHistoryOrders(contractCoinId), loadWallets()])
            triggerOrderRefresh()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitCancelOrder(orderId: string, contractCoinId?: number) {
        loading.value = true
        try {
            const res = await cancelOrder(orderId)
            await delay(500)
            await Promise.all([loadCurrentOrders(contractCoinId), loadWallets()])
            triggerOrderRefresh()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitCancelAllOrders(contractCoinId?: number) {
        loading.value = true
        try {
            const res = await cancelAllOrders(contractCoinId ? String(contractCoinId) : undefined)
            await delay(500)
            await Promise.all([loadCurrentOrders(contractCoinId), loadWallets()])
            triggerOrderRefresh()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitTransfer(params: TransferParams) {
        loading.value = true
        try {
            const res = await transferFunds(params)
            await delay(300)
            await loadWallets()
            return res.data
        } finally {
            loading.value = false
        }
    }

    async function submitModifyLeverage(contractCoinId: number, leverage: number, direction: 0 | 1 = 0) {
        const res = await modifyLeverage(contractCoinId, leverage, direction)
        return res.data
    }

    async function submitSwitchPattern(contractCoinId: number, targetPattern: 'cross' | 'isolated') {
        const res = await switchPattern(contractCoinId, targetPattern)
        return res.data
    }

    function setActiveCoin(coin: ContractCoin | null) {
        activeCoin.value = coin
    }

    function normalizeContractSymbol(symbol: string) {
        return symbol.replace(/[-_/]/g, '').toUpperCase()
    }

    function displayContractSymbol(symbol: string) {
        const normalized = normalizeContractSymbol(symbol)
        const quote = ['USDT', 'USDC', 'USD', 'BTC', 'ETH'].find(q => normalized.endsWith(q) && normalized.length > q.length)
        if (quote) return `${normalized.slice(0, -quote.length)}/${quote}`
        return symbol.replace(/[-_]/g, '/')
    }

    function firstFiniteNumber(values: unknown[], fallback: number) {
        for (const value of values) {
            if (value === null || value === undefined || value === '') continue
            const number = Number(value)
            if (Number.isFinite(number)) return number
        }
        return fallback
    }

    function getCoinBySymbol(symbol: string) {
        const normalized = normalizeContractSymbol(symbol)
        return coins.value.find(c => normalizeContractSymbol(c.symbol) === normalized)
    }

    function getThumbBySymbol(symbol: string) {
        const normalized = normalizeContractSymbol(symbol)
        return thumbs.value.find(t => normalizeContractSymbol(t.symbol) === normalized)
    }

    function updateThumb(data: any) {
        const rawSymbol = String(data.symbol ?? data.pair_id ?? data.pair ?? data.market ?? data.instId ?? '')
        if (!rawSymbol) return

        const normalized = normalizeContractSymbol(rawSymbol)
        const idx = thumbs.value.findIndex(t => normalizeContractSymbol(t.symbol) === normalized)
        const current = idx >= 0 ? thumbs.value[idx] : undefined
        const symbol = current?.symbol || getCoinBySymbol(rawSymbol)?.symbol || displayContractSymbol(rawSymbol)
        const close = firstFiniteNumber([data.close, data.last, data.price, data.last_price], current?.close ?? current?.last ?? 0)
        const open = firstFiniteNumber([data.open, data.open_24h], current?.open ?? close)
        const highFallback = current ? Math.max(current.high || close, close) : close
        const lowFallback = current?.low ? Math.min(current.low, close) : close
        const high = firstFiniteNumber([data.high, data.high_24h], highFallback)
        const low = firstFiniteNumber([data.low, data.low_24h], lowFallback)
        const vol = firstFiniteNumber([data.vol, data.volume, data.volume_24h], current?.vol ?? 0)
        const updated: CoinThumb = {
            symbol,
            open,
            close,
            last: firstFiniteNumber([data.last, data.close, data.price, data.last_price], close),
            high,
            low,
            vol,
            change: firstFiniteNumber(
                [data.change, data.chg, data.price_change_percent_24h],
                open ? ((close - open) / open * 100) : (current?.change ?? 0)
            )
        }
        if (idx >= 0) {
            thumbs.value[idx] = updated
        } else {
            thumbs.value.push(updated)
        }
    }

    function getAvailableBalance(unit: string = 'USDT') {
        const wallet = wallets.value.find(w => w.baseSymbol === unit)
        return wallet ? wallet.usdtBalance - wallet.usdtFrozenBalance : 0
    }

    return {
        coins,
        thumbs,
        currentOrders,
        historyOrders,
        wallets,
        activeCoin,
        activeSymbol,
        loading,
        loadCoins,
        loadThumbs,
        loadCurrentOrders,
        loadHistoryOrders,
        loadWallets,
        getWalletDetail,
        submitOpenPosition,
        submitClosePosition,
        submitCloseAllPositions,
        submitCancelOrder,
        submitCancelAllOrders,
        submitTransfer,
        submitModifyLeverage,
        submitSwitchPattern,
        orderRefreshKey,
        triggerOrderRefresh,
        setActiveCoin,
        getCoinBySymbol,
        getThumbBySymbol,
        updateThumb,
        getAvailableBalance
    }
})
