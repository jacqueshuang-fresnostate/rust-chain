import { publicMarketWebSocketUrl } from '@/config/app'
import { normalizeSymbol } from '@/core/format'
import { parseMarketSocketFrame, tickerSubscriptionFrame } from './marketSocketProtocol'

export interface TickerUpdate {
  symbol: string
  lastPrice: number
  observedAt?: number
}

type TickerListener = (update: TickerUpdate) => void

const RECONNECT_BASE_MS = 1_000
const RECONNECT_MAX_MS = 30_000
const HEARTBEAT_MS = 25_000

// 单连接多路订阅：按行情条数开连接会在列表页放大到几十条 WebSocket。
let socket: WebSocket | null = null
let reconnectAttempt = 0
let reconnectTimer: ReturnType<typeof setTimeout> | null = null
let heartbeatTimer: ReturnType<typeof setInterval> | null = null
const listeners = new Set<TickerListener>()
const subscribedSymbols = new Set<string>()

function sendSubscribe(symbol: string): void {
  if (socket?.readyState !== WebSocket.OPEN) return
  socket.send(tickerSubscriptionFrame(symbol))
}

function clearTimers(): void {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer)
    reconnectTimer = null
  }
  if (heartbeatTimer) {
    clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }
}

function scheduleReconnect(): void {
  if (reconnectTimer || listeners.size === 0) return
  const delay = Math.min(RECONNECT_BASE_MS * 2 ** reconnectAttempt, RECONNECT_MAX_MS)
  reconnectAttempt += 1
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null
    connect()
  }, delay)
}

function connect(): void {
  if (typeof window === 'undefined' || typeof window.WebSocket === 'undefined') return
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) return

  const next = new WebSocket(publicMarketWebSocketUrl())
  socket = next

  next.addEventListener('open', () => {
    reconnectAttempt = 0
    subscribedSymbols.forEach(sendSubscribe)
    heartbeatTimer = setInterval(() => {
      if (next.readyState === WebSocket.OPEN) next.send('ping')
    }, HEARTBEAT_MS)
  })

  next.addEventListener('message', (event) => {
    const frame = parseMarketSocketFrame(event.data)
    if (!frame || frame.type !== 'ticker') return
    const update: TickerUpdate = {
      symbol: normalizeSymbol(frame.symbol),
      lastPrice: frame.lastPrice,
      observedAt: frame.observedAt,
    }
    if (!subscribedSymbols.has(update.symbol)) return
    listeners.forEach((listener) => listener(update))
  })

  const onClosed = () => {
    clearTimers()
    if (socket === next) socket = null
    scheduleReconnect()
  }
  next.addEventListener('close', onClosed)
  next.addEventListener('error', onClosed)
}

/// 订阅实时价格；返回的函数用于停止接收并在无人订阅时关闭连接。
export function subscribeTickers(symbols: string[], listener: TickerListener): () => void {
  listeners.add(listener)
  symbols
    .map((symbol) => normalizeSymbol(symbol))
    .filter(Boolean)
    .forEach((symbol) => {
      const isNew = !subscribedSymbols.has(symbol)
      subscribedSymbols.add(symbol)
      if (isNew) sendSubscribe(symbol)
    })
  connect()

  return () => {
    listeners.delete(listener)
    if (listeners.size > 0) return
    clearTimers()
    subscribedSymbols.clear()
    reconnectAttempt = 0
    socket?.close()
    socket = null
  }
}
