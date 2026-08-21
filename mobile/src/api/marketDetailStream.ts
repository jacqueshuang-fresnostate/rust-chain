import type { KlinePoint, OrderBookLevel, TradePrint } from '../core/types.ts'
import {
  DEFAULT_MARKET_KLINE_LIMIT,
  depthSubscriptionFrame,
  klineSubscriptionFrame,
  mergeMarketKlines,
  normalizeMarketKlineInterval,
  normalizeMarketSocketSymbol,
  parseMarketSocketFrame,
  tradeSubscriptionFrame,
} from './marketSocketProtocol.ts'
import { createInboundSilenceWatchdog } from './webSocketLiveness.ts'

const SOCKET_CONNECTING = 0
const SOCKET_OPEN = 1
const RECONNECT_BASE_MS = 1_000
const RECONNECT_MAX_MS = 30_000
const HEARTBEAT_MS = 25_000
const INBOUND_IDLE_TIMEOUT_MS = 65_000
const DEFAULT_DETAIL_CHANNELS = ['depth', 'trade', 'kline'] as const

export type MarketDetailStreamChannel = (typeof DEFAULT_DETAIL_CHANNELS)[number]

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
  interval: string
  url: string
  channels?: readonly MarketDetailStreamChannel[]
  onDepth(snapshot: { bids: OrderBookLevel[]; asks: OrderBookLevel[] }): void
  onTrade(trade: TradePrint): void
  onKline(point: KlinePoint): void
  createSocket?: (url: string) => MarketDetailSocket | null
  scheduler?: MarketDetailStreamScheduler
  reconnectBaseMs?: number
  reconnectMaxMs?: number
  heartbeatMs?: number
  inboundIdleTimeoutMs?: number
}

export interface MarketDetailStreamContext {
  symbol: string
  interval: string
  requestVersion: number
  generation: number
  depthReceived: boolean
  tradeReceived: boolean
  klineReceived: boolean
}

export interface MarketDetailKlineRequest {
  context: MarketDetailStreamContext
  generation: number
}

export interface MarketDetailStreamSessionOptions {
  getUrl(): string
  channels?: readonly MarketDetailStreamChannel[]
  onDepth(
    context: MarketDetailStreamContext,
    snapshot: { bids: OrderBookLevel[]; asks: OrderBookLevel[] },
  ): void
  onTrade(context: MarketDetailStreamContext, trade: TradePrint): void
  onKlines(context: MarketDetailStreamContext, points: KlinePoint[]): void
  startStream?: (options: MarketDetailStreamOptions) => () => void
  klineLimit?: number
}

export interface MarketDetailStreamSession {
  replace(symbol: string, interval: string, requestVersion: number): MarketDetailStreamContext
  stop(): void
  current(): MarketDetailStreamContext | null
  currentPoints(): KlinePoint[]
  isCurrent(
    context: MarketDetailStreamContext,
    symbol?: string,
    interval?: string,
    requestVersion?: number,
  ): boolean
  beginKlineRequest(context: MarketDetailStreamContext): MarketDetailKlineRequest | null
  isCurrentKlineRequest(request: MarketDetailKlineRequest): boolean
  resolveKlineRequest(
    request: MarketDetailKlineRequest,
    restPoints: readonly KlinePoint[],
  ): KlinePoint[] | null
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
  const interval = normalizeMarketKlineInterval(options.interval)
  const channels = normalizeDetailChannels(options.channels)
  if (!symbol || !interval || !options.url.trim() || channels.size === 0) return () => undefined

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

  let active = true
  let socket: MarketDetailSocket | null = null
  let reconnectAttempt = 0
  let reconnectTimer: unknown = null
  let heartbeatTimer: unknown = null
  let depthFrame: unknown = null
  let depthFrameToken: object | null = null
  let pendingDepth: { bids: OrderBookLevel[]; asks: OrderBookLevel[] } | null = null
  let klineFrame: unknown = null
  let klineFrameToken: object | null = null
  let pendingKline: KlinePoint | null = null

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
    depthFrameToken = null
    if (depthFrame === null) return
    scheduler.cancelFrame(depthFrame)
    depthFrame = null
  }

  const clearPendingKline = (): void => {
    pendingKline = null
    klineFrameToken = null
    if (klineFrame === null) return
    scheduler.cancelFrame(klineFrame)
    klineFrame = null
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
      if (disconnected || socket !== next) return
      disconnected = true
      socket = null
      clearHeartbeat()
      inboundWatchdog.clear()
      clearPendingDepth()
      clearPendingKline()
      scheduleReconnect()
    }

    const closeAfterFailure = (): void => {
      if (!active || socket !== next || disconnected) return
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
        if (channels.has('depth')) next.send(depthSubscriptionFrame(symbol))
        if (channels.has('trade')) next.send(tradeSubscriptionFrame(symbol))
        if (channels.has('kline')) next.send(klineSubscriptionFrame(symbol, interval))
      } catch {
        closeAfterFailure()
        return
      }
      inboundWatchdog.arm(closeAfterFailure)
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
      inboundWatchdog.arm(closeAfterFailure)
      const frame = parseMarketSocketFrame(event.data)
      if (!frame || normalizeFrameSymbol(frame) !== symbol) return
      if (frame.type === 'depth' && channels.has('depth')) {
        pendingDepth = { bids: frame.bids, asks: frame.asks }
        if (depthFrameToken !== null) return
        const frameToken = {}
        depthFrameToken = frameToken
        const scheduledFrame = scheduler.requestFrame(() => {
          if (depthFrameToken !== frameToken) return
          depthFrameToken = null
          depthFrame = null
          const snapshot = pendingDepth
          pendingDepth = null
          if (!snapshot || !active || socket !== next || disconnected) return
          options.onDepth(snapshot)
        })
        if (depthFrameToken === frameToken) depthFrame = scheduledFrame
      } else if (frame.type === 'trade' && channels.has('trade')) {
        options.onTrade(frame.trade)
      } else if (frame.type === 'kline' && channels.has('kline') && frame.interval === interval) {
        pendingKline = frame.point
        if (klineFrameToken !== null) return
        const frameToken = {}
        klineFrameToken = frameToken
        const scheduledFrame = scheduler.requestFrame(() => {
          if (klineFrameToken !== frameToken) return
          klineFrameToken = null
          klineFrame = null
          const point = pendingKline
          pendingKline = null
          if (!point || !active || socket !== next || disconnected) return
          options.onKline(point)
        })
        if (klineFrameToken === frameToken) klineFrame = scheduledFrame
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
    inboundWatchdog.clear()
    clearPendingDepth()
    clearPendingKline()
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

export function createMarketDetailStreamSession(
  options: MarketDetailStreamSessionOptions,
): MarketDetailStreamSession {
  const startStream = options.startStream ?? startMarketDetailStream
  const klineLimit = options.klineLimit ?? DEFAULT_MARKET_KLINE_LIMIT
  let generation = 0
  let klineRequestGeneration = 0
  let currentContext: MarketDetailStreamContext | null = null
  let stopStream: (() => void) | null = null
  let points: KlinePoint[] = []

  const isCurrent = (
    context: MarketDetailStreamContext,
    symbol?: string,
    interval?: string,
    requestVersion?: number,
  ): boolean => {
    if (
      context !== currentContext
      || context.generation !== generation
      || (symbol !== undefined && context.symbol !== normalizeMarketSocketSymbol(symbol))
      || (interval !== undefined && context.interval !== normalizeMarketKlineInterval(interval))
      || (requestVersion !== undefined && context.requestVersion !== requestVersion)
    ) {
      return false
    }
    return true
  }

  const stop = (): void => {
    if (!currentContext && !stopStream) return
    generation += 1
    klineRequestGeneration += 1
    currentContext = null
    const previousStop = stopStream
    stopStream = null
    try {
      previousStop?.()
    } catch {
      return
    }
  }

  const replace = (
    symbol: string,
    interval: string,
    requestVersion: number,
  ): MarketDetailStreamContext => {
    stop()
    const context: MarketDetailStreamContext = {
      symbol: normalizeMarketSocketSymbol(symbol),
      interval: normalizeMarketKlineInterval(interval),
      requestVersion,
      generation: ++generation,
      depthReceived: false,
      tradeReceived: false,
      klineReceived: false,
    }
    klineRequestGeneration += 1
    currentContext = context
    points = []

    if (!context.symbol || !context.interval) return context
    try {
      stopStream = startStream({
        symbol: context.symbol,
        interval: context.interval,
        url: options.getUrl(),
        channels: options.channels,
        onDepth: (snapshot) => {
          if (!isCurrent(context)) return
          context.depthReceived = true
          options.onDepth(context, snapshot)
        },
        onTrade: (trade) => {
          if (!isCurrent(context)) return
          context.tradeReceived = true
          options.onTrade(context, trade)
        },
        onKline: (point) => {
          if (!isCurrent(context)) return
          context.klineReceived = true
          points = mergeMarketKlines([point], points, klineLimit)
          options.onKlines(context, [...points])
        },
      })
    } catch {
      stopStream = null
    }
    return context
  }

  const beginKlineRequest = (
    context: MarketDetailStreamContext,
  ): MarketDetailKlineRequest | null => {
    if (!isCurrent(context)) return null
    return {
      context,
      generation: ++klineRequestGeneration,
    }
  }

  const isCurrentKlineRequest = (request: MarketDetailKlineRequest): boolean => {
    return request.generation === klineRequestGeneration && isCurrent(request.context)
  }

  const resolveKlineRequest = (
    request: MarketDetailKlineRequest,
    restPoints: readonly KlinePoint[],
  ): KlinePoint[] | null => {
    if (!isCurrentKlineRequest(request)) return null
    points = mergeMarketKlines(points, restPoints, klineLimit)
    return [...points]
  }

  return {
    replace,
    stop,
    current: () => currentContext,
    currentPoints: () => [...points],
    isCurrent,
    beginKlineRequest,
    isCurrentKlineRequest,
    resolveKlineRequest,
  }
}

function normalizeFrameSymbol(
  frame: ReturnType<typeof parseMarketSocketFrame>,
): string {
  if (
    !frame
    || (
      frame.type !== 'ticker'
      && frame.type !== 'depth'
      && frame.type !== 'trade'
      && frame.type !== 'kline'
    )
  ) {
    return ''
  }
  return normalizeMarketSocketSymbol(frame.symbol)
}

function positiveDelay(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : fallback
}

function normalizeDetailChannels(
  channels: readonly MarketDetailStreamChannel[] | undefined,
): Set<MarketDetailStreamChannel> {
  return new Set(channels ?? DEFAULT_DETAIL_CHANNELS)
}
