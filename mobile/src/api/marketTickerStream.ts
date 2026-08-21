import {
  normalizeMarketSocketSymbol,
  parseMarketSocketFrame,
  tickerSubscriptionFrame,
  tickerUnsubscriptionFrame,
} from './marketSocketProtocol.ts'
import { createInboundSilenceWatchdog } from './webSocketLiveness.ts'

const SOCKET_CONNECTING = 0
const SOCKET_OPEN = 1
const RECONNECT_BASE_MS = 1_000
const RECONNECT_MAX_MS = 30_000
const HEARTBEAT_MS = 25_000
const INBOUND_IDLE_TIMEOUT_MS = 65_000

export interface TickerUpdate {
  symbol: string
  lastPrice: number
  highPrice?: number
  lowPrice?: number
  volume?: number
  changePercent?: number
  observedAt?: number
}

type TickerListener = (update: TickerUpdate) => void

interface TickerSocketEventMap {
  open: unknown
  message: { data: unknown }
  close: unknown
  error: unknown
}

export interface TickerSocket {
  readonly readyState: number
  send(data: string): void
  close(): void
  addEventListener<Type extends keyof TickerSocketEventMap>(
    type: Type,
    listener: (event: TickerSocketEventMap[Type]) => void,
  ): void
}

export interface TickerStreamScheduler {
  setTimeout(callback: () => void, delay: number): unknown
  clearTimeout(handle: unknown): void
  setInterval(callback: () => void, delay: number): unknown
  clearInterval(handle: unknown): void
}

export interface MarketTickerStreamOptions {
  getUrl(): string
  createSocket?: (url: string) => TickerSocket | null
  scheduler?: TickerStreamScheduler
  reconnectBaseMs?: number
  reconnectMaxMs?: number
  heartbeatMs?: number
  inboundIdleTimeoutMs?: number
}

export interface MarketTickerStream {
  subscribe(symbols: readonly string[], listener: TickerListener): () => void
}

interface TickerSubscription {
  symbols: Set<string>
  listener: TickerListener
}

const defaultScheduler: TickerStreamScheduler = {
  setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>),
  setInterval: (callback, delay) => globalThis.setInterval(callback, delay),
  clearInterval: (handle) => globalThis.clearInterval(handle as ReturnType<typeof globalThis.setInterval>),
}

function createBrowserSocket(url: string): TickerSocket | null {
  if (typeof globalThis.WebSocket === 'undefined') return null
  return new globalThis.WebSocket(url) as unknown as TickerSocket
}

export function createMarketTickerStream(options: MarketTickerStreamOptions): MarketTickerStream {
  const scheduler = options.scheduler ?? defaultScheduler
  const createSocket = options.createSocket ?? createBrowserSocket
  const reconnectBaseMs = positiveDelay(options.reconnectBaseMs, RECONNECT_BASE_MS)
  const reconnectMaxMs = Math.max(
    reconnectBaseMs,
    positiveDelay(options.reconnectMaxMs, RECONNECT_MAX_MS),
  )
  const heartbeatMs = positiveDelay(options.heartbeatMs, HEARTBEAT_MS)
  const inboundIdleTimeoutMs = positiveDelay(
    options.inboundIdleTimeoutMs,
    INBOUND_IDLE_TIMEOUT_MS,
  )
  const inboundWatchdog = createInboundSilenceWatchdog(scheduler, inboundIdleTimeoutMs)
  const subscriptions = new Set<TickerSubscription>()
  const socketSymbols = new Set<string>()
  let socket: TickerSocket | null = null
  let reconnectAttempt = 0
  let reconnectTimer: unknown = null
  let heartbeatTimer: unknown = null

  const desiredSymbols = (): Set<string> => {
    const symbols = new Set<string>()
    subscriptions.forEach((subscription) => {
      subscription.symbols.forEach((symbol) => symbols.add(symbol))
    })
    return symbols
  }

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

  const closeCurrentSocket = (): void => {
    clearReconnect()
    clearHeartbeat()
    inboundWatchdog.clear()
    socketSymbols.clear()
    reconnectAttempt = 0
    const current = socket
    socket = null
    if (!current) return
    try {
      current.close()
    } catch {
      return
    }
  }

  const scheduleReconnect = (): void => {
    if (!subscriptions.size || reconnectTimer !== null) return
    const delay = Math.min(reconnectBaseMs * 2 ** reconnectAttempt, reconnectMaxMs)
    reconnectAttempt = Math.min(reconnectAttempt + 1, 30)
    reconnectTimer = scheduler.setTimeout(() => {
      reconnectTimer = null
      connect()
    }, delay)
  }

  const syncSubscriptions = (current: TickerSocket): boolean => {
    if (socket !== current || current.readyState !== SOCKET_OPEN) return false
    const desired = desiredSymbols()
    try {
      for (const symbol of socketSymbols) {
        if (!desired.has(symbol)) current.send(tickerUnsubscriptionFrame(symbol))
      }
      for (const symbol of desired) {
        if (!socketSymbols.has(symbol)) current.send(tickerSubscriptionFrame(symbol))
      }
    } catch {
      return false
    }
    socketSymbols.clear()
    desired.forEach((symbol) => socketSymbols.add(symbol))
    return true
  }

  const connect = (): void => {
    if (
      !subscriptions.size
      || (socket && (socket.readyState === SOCKET_CONNECTING || socket.readyState === SOCKET_OPEN))
    ) {
      return
    }

    let next: TickerSocket | null
    try {
      next = createSocket(options.getUrl())
    } catch {
      scheduleReconnect()
      return
    }
    if (!next) return

    socket = next
    let disconnected = false

    const disconnect = (closeSocket: boolean): void => {
      if (disconnected) return
      disconnected = true
      if (socket !== next) return
      socket = null
      socketSymbols.clear()
      clearHeartbeat()
      inboundWatchdog.clear()
      if (closeSocket) {
        try {
          next.close()
        } catch {
          // The connection is already detached; reconnect scheduling still proceeds.
        }
      }
      scheduleReconnect()
    }

    next.addEventListener('open', () => {
      if (socket !== next || disconnected || !subscriptions.size) {
        try {
          next.close()
        } catch {
          return
        }
        return
      }
      reconnectAttempt = 0
      clearHeartbeat()
      socketSymbols.clear()
      if (!syncSubscriptions(next)) {
        disconnect(true)
        return
      }
      inboundWatchdog.arm(() => disconnect(true))
      heartbeatTimer = scheduler.setInterval(() => {
        if (socket !== next || disconnected || next.readyState !== SOCKET_OPEN) return
        try {
          next.send('ping')
        } catch {
          disconnect(true)
        }
      }, heartbeatMs)
    })

    next.addEventListener('message', (event) => {
      if (socket !== next || disconnected) return
      inboundWatchdog.arm(() => disconnect(true))
      const frame = parseMarketSocketFrame(event.data)
      if (!frame || frame.type !== 'ticker') return
      const update: TickerUpdate = {
        symbol: normalizeMarketSocketSymbol(frame.symbol),
        lastPrice: frame.lastPrice,
        ...(frame.highPrice === undefined ? {} : { highPrice: frame.highPrice }),
        ...(frame.lowPrice === undefined ? {} : { lowPrice: frame.lowPrice }),
        ...(frame.volume === undefined ? {} : { volume: frame.volume }),
        ...(frame.changePercent === undefined ? {} : { changePercent: frame.changePercent }),
        observedAt: frame.observedAt,
      }
      for (const subscription of [...subscriptions]) {
        if (!subscription.symbols.has(update.symbol)) continue
        try {
          subscription.listener(update)
        } catch {
          // One view listener must not prevent other active leases from receiving the tick.
        }
      }
    })

    next.addEventListener('close', () => disconnect(false))
    next.addEventListener('error', () => disconnect(true))
  }

  const subscribe = (symbols: readonly string[], listener: TickerListener): (() => void) => {
    const normalizedSymbols = new Set(
      symbols.map(normalizeMarketSocketSymbol).filter(Boolean),
    )
    if (!normalizedSymbols.size) return () => undefined

    const subscription: TickerSubscription = {
      symbols: normalizedSymbols,
      listener,
    }
    subscriptions.add(subscription)
    connect()
    if (socket?.readyState === SOCKET_OPEN && !syncSubscriptions(socket)) {
      const failedSocket = socket
      socket = null
      socketSymbols.clear()
      clearHeartbeat()
      inboundWatchdog.clear()
      try {
        failedSocket.close()
      } catch {
        // Reconnect below owns recovery from a synchronous send failure.
      }
      scheduleReconnect()
    }

    let active = true
    return () => {
      if (!active) return
      active = false
      subscriptions.delete(subscription)
      if (!subscriptions.size) {
        closeCurrentSocket()
        return
      }
      if (socket?.readyState === SOCKET_OPEN && !syncSubscriptions(socket)) {
        const failedSocket = socket
        socket = null
        socketSymbols.clear()
        clearHeartbeat()
        inboundWatchdog.clear()
        try {
          failedSocket.close()
        } catch {
          // Reconnect below owns recovery from a synchronous send failure.
        }
        scheduleReconnect()
      }
    }
  }

  return { subscribe }
}

function positiveDelay(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : fallback
}
