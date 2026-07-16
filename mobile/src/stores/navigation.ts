import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { DEFAULT_TRADE_SYMBOL, normalizeRouteSymbol } from '@/core/navigation'

const LAST_TRADE_SYMBOL_KEY = 'hippo_mobile_last_trade_symbol'
const LAST_TRADE_MODE_KEY = 'hippo_mobile_last_trade_mode'

function readLastTradeSymbol(): string {
  try {
    return normalizeRouteSymbol(globalThis.localStorage?.getItem(LAST_TRADE_SYMBOL_KEY))
  } catch {
    return DEFAULT_TRADE_SYMBOL
  }
}

function readLastTradeMode(): 'spot' | 'contract' {
  try {
    return globalThis.localStorage?.getItem(LAST_TRADE_MODE_KEY) === 'contract' ? 'contract' : 'spot'
  } catch {
    return 'spot'
  }
}

export const useNavigationStore = defineStore('mobile-navigation', () => {
  const lastTradeSymbol = ref(readLastTradeSymbol())
  const lastTradeMode = ref<'spot' | 'contract'>(readLastTradeMode())
  const lastTradePath = computed(() => `/trade/${lastTradeSymbol.value}${lastTradeMode.value === 'contract' ? '?mode=contract' : ''}`)

  function rememberTradeSymbol(symbol: unknown): void {
    lastTradeSymbol.value = normalizeRouteSymbol(symbol)
    try {
      globalThis.localStorage?.setItem(LAST_TRADE_SYMBOL_KEY, lastTradeSymbol.value)
    } catch {
      // 存储不可用时仍保留当前运行周期内的最近交易对。
    }
  }

  function rememberTradeMode(mode: unknown): void {
    lastTradeMode.value = mode === 'contract' ? 'contract' : 'spot'
    try {
      globalThis.localStorage?.setItem(LAST_TRADE_MODE_KEY, lastTradeMode.value)
    } catch {
      // 存储不可用时仍保留当前运行周期内的最近交易模式。
    }
  }

  return { lastTradeSymbol, lastTradeMode, lastTradePath, rememberTradeSymbol, rememberTradeMode }
})
