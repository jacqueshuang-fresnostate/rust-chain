import type { OrderBookLevel, TradePrint } from '../core/types.ts'
import {
  depthSubscriptionFrame,
  normalizeMarketSocketSymbol,
  parseMarketSocketFrame,
  tradeSubscriptionFrame,
} from './marketSocketProtocol.ts'

const SOCKET_CONNECTING = 0
const SOCKET_OPEN = 1
const RECONNECT_BASE_MS = 1_000
const RECONNECT_MAX_MS = 30_000
const HEARTBEAT_MS = 25_000

interface MarketDetailSocketEventMap {
  open: unknown
  message: { data: unknown }
  close: unknown
  error: unknown
}

export interface MarketDetailSocket {
  readonly readyState: number
  send(data: string): void
  close(): void
  addEventListener<Type extends keyof MarketDetailSocketEventMap>(
    type: Type,
    listener: (event: MarketDetailSocketEventMap[Type]) => void,
  ): void
}

export interface MarketDetailStreamScheduler {
  setTimeout(callback: () => void, delay: number): unknown
  clearTimeout(handle: unknown): void
  setInterval(callback: () => void, delay: number): unknown
  clearInterval(handle: unknown): void
  requestFrame(callback: () => void): unknown
  cancelFrame(handle: unknown): void
}

export interface MarketDetailStreamOptions {
  symbol: string
  url: string
  onDepth(snapshot: { bids: OrderBookLevel[]; asks: OrderBookLevel[] }): void
  onTrade(trade: TradePrint): void
  createSocket?: (url: string) => MarketDetailSocket | null
  scheduler?: MarketDetailStreamScheduler
  reconnectBaseMs?: number
  reconnectMaxMs?: number
  heartbeatMs?: number
}

const defaultScheduler: MarketDetailStreamScheduler = {
  setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>),
  setInterval: (callback, delay) => globalThis.setInterval(callback, delay),
  clearInterval: (handle) => globalThis.clearInterval(handle as ReturnType<typeof globalThis.setInterval>),
  requestFrame: (callback) => (
    typeof globalThis.requestAnimationFrame === 'function'
      ? globalThis.requestAnimationFrame(() => callback())
      : globalThis.setTimeout(callback, 16)
  ),
  cancelFrame: (handle) => {
    if (typeof globalThis.cancelAnimationFrame === 'function') {
      globalThis.cancelAnimationFrame(handle as number)
      return
    }
    globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>)
  },
}

function createBrowserSocket(url: string): MarketDetailSocket | null {
  if (typeof window === 'undefined' || typeof window.WebSocket === 'undefined') return null
  return new window.WebSocket(url) as unknown as MarketDetailSocket
}

export function startMarketDetailStream(options: MarketDetailStreamOptions): () => void {
  const symbol = normalizeMarketSocketSymbol(options.symbol)
  if (!symbol || !options.url.trim()) return () => undefined

  const scheduler = options.scheduler ?? defaultScheduler
  const createSocket = options.createSocket ?? createBrowserSocket
  const reconnectBaseMs = positiveDelay(options.reconnectBaseMs, RECONNECT_BASE_MS)
  const reconnectMaxMs = Math.max(
    reconnectBaseMs,
    positiveDelay(options.reconnectMaxMs, RECONNECT_MAX_MS),
  )
  const heartbeatMs = positiveDelay(options.heartbeatMs, HEARTBEAT_MS)

  let active = true
  let socket: MarketDetailSocket | null = null
  let reconnectAttempt = 0
  let reconnectTimer: unknown = null
  let heartbeatTimer: unknown = null
  let depthFrame: unknown = null
  let pendingDepth: { bids: OrderBookLevel[]; asks: OrderBookLevel[] } | null = null

  const clearHeartbeat = (): void => {
    if (heartbeatTimer === null) return
    scheduler.clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }

  const clearReconnect = (): void => {
    if (reconnectTimer === null) return
    scheduler.clearTimeout(reconnectTimer)
    reconnectTimer = null
  }

  const clearPendingDepth = (): void => {
    pendingDepth = null
    if (depthFrame === null) return
    scheduler.cancelFrame(depthFrame)
    depthFrame = null
  }

  const scheduleReconnect = (): void => {
    if (!active || reconnectTimer !== null) return
    const delay = Math.min(reconnectBaseMs * 2 ** reconnectAttempt, reconnectMaxMs)
    reconnectAttempt = Math.min(reconnectAttempt + 1, 30)
    reconnectTimer = scheduler.setTimeout(() => {
      reconnectTimer = null
      connect()
    }, delay)
  }

  const connect = (): void => {
    if (
      !active
      || (socket && (socket.readyState === SOCKET_CONNECTING || socket.readyState === SOCKET_OPEN))
    ) {
      return
    }

    let next: MarketDetailSocket | null
    try {
      next = createSocket(options.url)
    } catch {
      scheduleReconnect()
      return
    }
    if (!next) return

    socket = next
    let disconnected = false

    const disconnect = (): void => {
      if (disconnected) return
      disconnected = true
      if (socket === next) socket = null
      clearHeartbeat()
      clearPendingDepth()
      scheduleReconnect()
    }

    const closeAfterFailure = (): void => {
      disconnect()
      try {
        next.close()
      } catch {
        return
      }
    }

    next.addEventListener('open', () => {
      if (!active || socket !== next || disconnected) {
        try {
          next.close()
        } catch {
          return
        }
        return
      }

      reconnectAttempt = 0
      clearHeartbeat()
      try {
        next.send(depthSubscriptionFrame(symbol))
        next.send(tradeSubscriptionFrame(symbol))
      } catch {
        closeAfterFailure()
        return
      }
      heartbeatTimer = scheduler.setInterval(() => {
        if (!active || socket !== next || next.readyState !== SOCKET_OPEN) return
        try {
          next.send('ping')
        } catch {
          closeAfterFailure()
        }
      }, heartbeatMs)
    })

    next.addEventListener('message', (event) => {
      if (!active || socket !== next || disconnected) return
      const frame = parseMarketSocketFrame(event.data)
      if (!frame || normalizeFrameSymbol(frame) !== symbol) return
      if (frame.type === 'depth') {
        pendingDepth = { bids: frame.bids, asks: frame.asks }
        if (depthFrame !== null) return
        depthFrame = scheduler.requestFrame(() => {
          depthFrame = null
          const snapshot = pendingDepth
          pendingDepth = null
          if (!snapshot || !active || socket !== next || disconnected) return
          options.onDepth(snapshot)
        })
      } else if (frame.type === 'trade') {
        options.onTrade(frame.trade)
      }
    })

    next.addEventListener('close', disconnect)
    next.addEventListener('error', closeAfterFailure)
  }

  connect()

  return () => {
    if (!active) return
    active = false
    clearReconnect()
    clearHeartbeat()
    clearPendingDepth()
    const current = socket
    socket = null
    if (!current) return
    try {
      current.close()
    } catch {
      return
    }
  }
}

function normalizeFrameSymbol(
  frame: ReturnType<typeof parseMarketSocketFrame>,
): string {
  if (!frame || (frame.type !== 'ticker' && frame.type !== 'depth' && frame.type !== 'trade')) {
    return ''
  }
  return normalizeMarketSocketSymbol(frame.symbol)
}

function positiveDelay(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : fallback
}
