import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
    fetchSecondSnapshot,
    fetchSecondCycles,
    fetchSecondCurrentOrders,
    fetchSecondHistoryOrders,
    fetchSecondOrderResult,
    fetchSecondBalance,
    submitSecondOrder,
    transferSecondFunds,
    type SecondOrderParams,
    type SecondTransferParams
} from '@/api/second'

export interface Ticker {
    symbol: string
    icon: string
    open: number
    high: number
    low: number
    close: number
    volume: number
    turnover: number
    time: number
    chg: number
    zone: number
}

export interface SecondCycle {
    id: number
    productId: number
    symbol: string
    cycleLength: number   // duration in seconds
    cycleRate: number     // payout rate, e.g. 0.85
    minAmount: number
    maxAmount: number
}

export interface SecondOrder {
    id: number
    symbol: string
    coinSymbol: string
    direction: 'BUY' | 'SELL'       // 0=涨, 1=跌
    betAmount: number
    amount: number
    openPrice: number
    closePrice: number
    cycleLength: number
    cycleRate: number
    status: string | number // 'ENTRUST' | 'OPEN' | 'CLOSE' | 'CANCELED'
    result: string | number // 'WIN'=盈利 'LOSE'=亏损 (or numeric fallback)
    profit: number
    createTime: number
    endTime: number
}

export const useSecondStore = defineStore('second', () => {
    const activeSymbol = ref<string>('BTC/USDT')
    const tickers = ref<Ticker[]>([])
    const cycles = ref<SecondCycle[]>([])
    const currentOrders = ref<SecondOrder[]>([])
    const historyOrders = ref<SecondOrder[]>([])
    const historyPage = ref<number>(0)
    const historyHasMore = ref<boolean>(true)
    const loadingHistory = ref<boolean>(false)
    const balance = ref<number>(0)
    const loading = ref(false)
    const orderRefreshKey = ref<number>(0)

    // ========== Ticker ==========

    function setActiveSymbol(symbol: string) {
        activeSymbol.value = symbol
    }

    function setTickers(newTickers: any[]) {
        tickers.value = newTickers.map(t => ({
            symbol: t.symbol,
            open: Number(t.open) || 0,
            high: Number(t.high) || 0,
            low: Number(t.low) || 0,
            close: Number(t.close) || 0,
            volume: Number(t.volume) || 0,
            turnover: Number(t.turnover) || 0,
            icon: typeof t.icon === 'string' ? t.icon : '',
            time: t.time || 0,
            chg: t.open ? ((Number(t.close) - Number(t.open)) / Number(t.open) * 100) : 0,
            zone: t.zone || 0
        }))
    }

    function compactTickerSymbol(symbol: string) {
        return symbol.replace(/[-_/]/g, '').toUpperCase()
    }

    function displayTickerSymbol(symbol: string) {
        const normalized = compactTickerSymbol(symbol)
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

    function updateTicker(ticker: any) {
        const rawSymbol = String(ticker.symbol ?? ticker.pair_id ?? ticker.pair ?? ticker.market ?? ticker.instId ?? '')
        if (!rawSymbol) return

        const normalized = compactTickerSymbol(rawSymbol)
        const index = tickers.value.findIndex(t => compactTickerSymbol(t.symbol) === normalized)
        const current = index >= 0 ? tickers.value[index] : undefined
        const close = firstFiniteNumber([ticker.close, ticker.last, ticker.price, ticker.last_price], current?.close ?? 0)
        const open = firstFiniteNumber([ticker.open, ticker.open_24h], current?.open ?? close)
        const highFallback = current ? Math.max(current.high || close, close) : close
        const lowFallback = current?.low ? Math.min(current.low, close) : close
        const high = firstFiniteNumber([ticker.high, ticker.high_24h], highFallback)
        const low = firstFiniteNumber([ticker.low, ticker.low_24h], lowFallback)
        const volume = firstFiniteNumber([ticker.volume, ticker.vol, ticker.volume_24h], current?.volume ?? 0)
        const updated: Ticker = {
            symbol: current?.symbol || displayTickerSymbol(rawSymbol),
            open,
            close,
            high,
            low,
            volume,
            turnover: firstFiniteNumber([ticker.turnover], current?.turnover ?? close * volume),
            icon: ticker.icon || current?.icon || '',
            time: firstFiniteNumber([ticker.time, ticker.observed_at], current?.time ?? 0),
            chg: firstFiniteNumber(
                [ticker.chg, ticker.change, ticker.price_change_percent_24h],
                open ? ((close - open) / open * 100) : (current?.chg ?? 0)
            ),
            zone: firstFiniteNumber([ticker.zone], current?.zone ?? 0)
        }
        if (index >= 0) {
            tickers.value[index] = updated
        } else {
            tickers.value.push(updated)
        }
    }

    function getTickerBySymbol(symbol: string) {
        const normalized = compactTickerSymbol(symbol)
        return tickers.value.find(t => compactTickerSymbol(t.symbol) === normalized)
    }

    // ========== Cycles ==========

    async function loadCycles() {
        try {
            const res = await fetchSecondCycles()
            const body = res.data
            const list = body?.data || (Array.isArray(body) ? body : [])
            cycles.value = list.map((c: any) => ({
                id: c.id,
                productId: Number(c.productId ?? c.product_id ?? c.id) || 0,
                symbol: c.symbol || '',
                cycleLength: Number(c.cycleLength) || 0,
                cycleRate: Number(c.cycleRate) || 0,
                minAmount: Number(c.minAmount) || 0,
                maxAmount: Number(c.maxAmount) || 0
            }))
        } catch (e) {
            console.error('loadCycles error', e)
        }
    }

    // ========== Orders ==========

    async function loadCurrentOrders(symbol: string) {
        try {
            const res = await fetchSecondCurrentOrders(symbol)
            const body = res.data
            const list = body?.data || (Array.isArray(body) ? body : [])
            currentOrders.value = mapOrders(list)
        } catch (e) {
            console.error('loadCurrentOrders error', e)
        }
    }

    async function loadHistoryOrders(symbol?: string, page: number = 0, pageSize: number = 50) {
        if (loadingHistory.value) return
        if (page === 0) {
            historyHasMore.value = true
        } else if (!historyHasMore.value) {
            return
        }

        loadingHistory.value = true
        try {
            const res = await fetchSecondHistoryOrders({
                symbol,
                pageNo: page,
                pageSize
            })
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

            const newOrders = mapOrders(list)
            if (page === 0) {
                historyOrders.value = newOrders
            } else {
                historyOrders.value = [...historyOrders.value, ...newOrders]
            }
            historyPage.value = page

            // Assume no more data if returned less than requested
            if (newOrders.length < pageSize) {
                historyHasMore.value = false
            }
        } catch (e) {
            console.error('loadHistoryOrders error', e)
        } finally {
            loadingHistory.value = false
        }
    }

    async function checkOrderResult(id: number, symbol: string) {
        try {
            const res = await fetchSecondOrderResult(id, symbol)
            const body = res.data
            const order = body?.data || body
            return order
        } catch (e) {
            console.error('checkOrderResult error', e)
            return null
        }
    }

    function mapOrders(list: any[]): SecondOrder[] {
        return list.map((o: any) => ({
            id: o.id || o.orderId,
            symbol: o.symbol || '',
            coinSymbol: o.coinSymbol || '',
            direction: o.direction ?? 0,
            amount: Number(o.amount ?? o.betAmount) || 0,
            betAmount: Number(o.betAmount ?? o.amount) || 0,
            openPrice: Number(o.openPrice ?? o.entrustPrice) || 0,
            closePrice: Number(o.closePrice ?? o.resultPrice) || 0,
            cycleLength: Number(o.cycleLength ?? o.seconds) || 0,
            cycleRate: Number(o.cycleRate ?? o.rewardPercent) || 0,
            status: o.status ?? 0,
            result: o.result ?? o.betResult ?? 0,
            profit: Number(o.winAmount ?? o.profit ?? o.rewardAmount) || 0,
            createTime: o.createTime || o.createDate || 0,
            endTime: o.endTime || (o.createTime ? o.createTime + (Number(o.cycleLength ?? o.seconds) || 0) * 1000 : 0)
        }))
    }

    // ========== Submit Order ==========

    async function placeOrder(params: SecondOrderParams) {
        loading.value = true
        try {
            const res = await submitSecondOrder(params)
            // Refresh after placing order
            await Promise.all([
                loadCurrentOrders(params.symbol),
                loadBalance()
            ])
            orderRefreshKey.value++
            return res.data
        } finally {
            loading.value = false
        }
    }

    // ========== Balance ==========

    async function loadBalance() {
        try {
            const res = await fetchSecondBalance('USDT-ERC20')
            const body = res.data
            balance.value = Number(body?.data ?? body) || 0
        } catch (e) {
            console.error('loadBalance error', e)
        }
    }

    // ========== Transfer ==========

    async function transfer(params: SecondTransferParams) {
        loading.value = true
        try {
            const res = await transferSecondFunds(params)
            await loadBalance()
            return res.data
        } finally {
            loading.value = false
        }
    }

    // ========== Tickers Loading ==========

    async function loadTickers() {
        try {
            const res = await fetchSecondSnapshot()
            const body = res.data
            const data = body?.data || (Array.isArray(body) ? body : [])
            setTickers(data)
        } catch (e) {
            console.error('loadTickers error', e)
        }
    }

    return {
        activeSymbol,
        tickers,
        cycles,
        currentOrders,
        historyOrders,
        historyPage,
        historyHasMore,
        loadingHistory,
        balance,
        loading,
        orderRefreshKey,
        setActiveSymbol,
        setTickers,
        updateTicker,
        getTickerBySymbol,
        loadTickers,
        loadCycles,
        loadCurrentOrders,
        loadHistoryOrders,
        checkOrderResult,
        placeOrder,
        loadBalance,
        transfer
    }
}, {
    persist: {
        paths: ['activeSymbol']
    }
})
