const SOCKET_OPEN = 1

export const PRIVATE_USER_RECONNECT_BASE_MS = 1_000
export const PRIVATE_USER_RECONNECT_MAX_MS = 30_000
export const PRIVATE_USER_RECONNECT_JITTER_RATIO = 0.2
export const PRIVATE_USER_HEARTBEAT_MS = 25_000
export const PRIVATE_USER_INBOUND_IDLE_TIMEOUT_MS = 65_000

const IGNORED_FRAME_TYPES = new Set([
  'pong',
  'subscribed',
  'unsubscribed',
  'subscription.confirmed',
  'subscription_confirmation',
  'error',
])

export type PrivateUserTransportState = 'connecting' | 'live' | 'stale' | 'stopped'

export type PrivateUserEvent = Readonly<Record<string, unknown>> & {
  readonly type: string
}

interface PrivateUserSocketEventMap {
  open: unknown
  message: { data: unknown }
  close: unknown
  error: unknown
}

export interface PrivateUserSocket {
  readonly readyState: number
  send(data: string): void
  close(): void
  addEventListener<Type extends keyof PrivateUserSocketEventMap>(
    type: Type,
    listener: (event: PrivateUserSocketEventMap[Type]) => void,
  ): void
}

export interface PrivateUserStreamScheduler {
  setTimeout(callback: () => void, delay: number): unknown
  clearTimeout(handle: unknown): void
  setInterval(callback: () => void, delay: number): unknown
  clearInterval(handle: unknown): void
}

export interface PrivateUserStreamOptions {
  getAccessToken(): string
  getUrl(accessToken: string): string | null
  onOpen?(): void
  /** Called for every inbound frame before protocol/business parsing. */
  onInboundFrame?(receivedAt: number): void
  onStateChange?(state: PrivateUserTransportState): void
  onEvent(event: PrivateUserEvent): void
  createSocket?: (url: string) => PrivateUserSocket | null
  scheduler?: PrivateUserStreamScheduler
  now?: () => number
  random?: () => number
  reconnectBaseMs?: number
  reconnectMaxMs?: number
  reconnectJitterRatio?: number
  heartbeatMs?: number
  inboundIdleTimeoutMs?: number
}

export interface PrivateUserStreamSession {
  start(): boolean
  stop(): void
  isRunning(): boolean
}

const defaultScheduler: PrivateUserStreamScheduler = {
  setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: (handle) => globalThis.clearTimeout(
    handle as ReturnType<typeof globalThis.setTimeout>,
  ),
  setInterval: (callback, delay) => globalThis.setInterval(callback, delay),
  clearInterval: (handle) => globalThis.clearInterval(
    handle as ReturnType<typeof globalThis.setInterval>,
  ),
}

function createBrowserSocket(url: string): PrivateUserSocket | null {
  if (typeof globalThis.WebSocket === 'undefined') return null
  return new globalThis.WebSocket(url) as unknown as PrivateUserSocket
}

export function parsePrivateUserFrame(data: unknown): PrivateUserEvent | null {
  if (typeof data !== 'string') return null
  const text = data.trim()
  if (!text || text.toLowerCase() === 'pong') return null

  let payload: unknown
  try {
    payload = JSON.parse(text)
  } catch {
    return null
  }
  if (!isRecord(payload)) return null

  const type = typeof payload.type === 'string' ? payload.type.trim() : ''
  if (!type || IGNORED_FRAME_TYPES.has(type.toLowerCase())) return null
  return { ...payload, type }
}

/**
 * Owns one authenticated private-user transport. Session-generation ownership
 * lives in the shared manager; this transport owns socket identity, heartbeat,
 * inbound liveness, and bounded reconnect work for that one generation.
 */
export function createPrivateUserStream(
  options: PrivateUserStreamOptions,
): PrivateUserStreamSession {
  const scheduler = options.scheduler ?? defaultScheduler
  const createSocket = options.createSocket ?? createBrowserSocket
  const now = options.now ?? Date.now
  const random = options.random ?? Math.random
  const reconnectBaseMs = positiveDelay(
    options.reconnectBaseMs,
    PRIVATE_USER_RECONNECT_BASE_MS,
  )
  const reconnectMaxMs = Math.max(
    reconnectBaseMs,
    positiveDelay(options.reconnectMaxMs, PRIVATE_USER_RECONNECT_MAX_MS),
  )
  const reconnectJitterRatio = normalizedRatio(
    options.reconnectJitterRatio,
    PRIVATE_USER_RECONNECT_JITTER_RATIO,
  )
  const heartbeatMs = positiveDelay(options.heartbeatMs, PRIVATE_USER_HEARTBEAT_MS)
  const inboundIdleTimeoutMs = positiveDelay(
    options.inboundIdleTimeoutMs,
    PRIVATE_USER_INBOUND_IDLE_TIMEOUT_MS,
  )

  let running = false
  let lifecycleLease: object | null = null
  let socket: PrivateUserSocket | null = null
  let state: PrivateUserTransportState = 'stopped'
  let reconnectAttempt = 0
  let reconnectTimer: unknown = null
  let reconnectTimerLease: object | null = null
  let heartbeatTimer: unknown = null
  let heartbeatTimerLease: object | null = null
  let inboundWatchdogTimer: unknown = null
  let inboundWatchdogLease: object | null = null

  const notifyState = (next: PrivateUserTransportState): void => {
    if (state === next) return
    state = next
    try {
      options.onStateChange?.(next)
    } catch {
      // Diagnostics cannot own or interrupt the transport lifecycle.
    }
  }

  const readLatestAccessToken = (): string => {
    try {
      return options.getAccessToken().trim()
    } catch {
      return ''
    }
  }

  const clearReconnect = (): void => {
    reconnectTimerLease = null
    if (reconnectTimer === null) return
    scheduler.clearTimeout(reconnectTimer)
    reconnectTimer = null
  }

  const clearHeartbeat = (): void => {
    heartbeatTimerLease = null
    if (heartbeatTimer === null) return
    scheduler.clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }

  const clearInboundWatchdog = (): void => {
    inboundWatchdogLease = null
    if (inboundWatchdogTimer === null) return
    scheduler.clearTimeout(inboundWatchdogTimer)
    inboundWatchdogTimer = null
  }

  const deactivateForMissingToken = (lease: object): void => {
    if (lifecycleLease !== lease) return
    running = false
    lifecycleLease = null
    clearReconnect()
    clearHeartbeat()
    clearInboundWatchdog()
    notifyState('stopped')
  }

  const scheduleReconnect = (lease: object): void => {
    if (
      !running
      || lifecycleLease !== lease
      || reconnectTimerLease !== null
    ) return
    if (!readLatestAccessToken()) {
      deactivateForMissingToken(lease)
      return
    }

    const exponentialDelay = Math.min(
      reconnectBaseMs * 2 ** reconnectAttempt,
      reconnectMaxMs,
    )
    const delay = jitteredDelay(
      exponentialDelay,
      reconnectMaxMs,
      reconnectJitterRatio,
      safeRandom(random),
    )
    reconnectAttempt = Math.min(reconnectAttempt + 1, 30)
    const timerLease = {}
    reconnectTimerLease = timerLease
    reconnectTimer = scheduler.setTimeout(() => {
      if (
        reconnectTimerLease !== timerLease
        || !running
        || lifecycleLease !== lease
      ) return
      reconnectTimer = null
      reconnectTimerLease = null
      notifyState('connecting')
      connect(lease)
    }, delay)
  }

  const connect = (lease: object): void => {
    if (!running || lifecycleLease !== lease || socket !== null) return

    const accessToken = readLatestAccessToken()
    if (!accessToken) {
      deactivateForMissingToken(lease)
      return
    }

    let url: string | null
    try {
      url = options.getUrl(accessToken)
    } catch {
      scheduleReconnect(lease)
      return
    }
    if (!url?.trim()) {
      deactivateForMissingToken(lease)
      return
    }

    let next: PrivateUserSocket | null
    try {
      next = createSocket(url)
    } catch {
      scheduleReconnect(lease)
      return
    }
    if (!next) {
      deactivateForMissingToken(lease)
      return
    }

    socket = next
    const connectionLease = {}
    let disconnected = false
    const isCurrentConnection = (): boolean => (
      running
      && lifecycleLease === lease
      && socket === next
      && !disconnected
    )

    const disconnect = (
      closeSocket: boolean,
      nextState: 'connecting' | 'stale',
    ): void => {
      if (!isCurrentConnection()) return
      disconnected = true
      socket = null
      clearHeartbeat()
      clearInboundWatchdog()
      notifyState(nextState)
      if (closeSocket) {
        try {
          next.close()
        } catch {
          // Reconnect scheduling remains authoritative after a failed close.
        }
      }
      scheduleReconnect(lease)
    }

    const armInboundWatchdog = (): void => {
      if (!isCurrentConnection()) return
      clearInboundWatchdog()
      const watchdogLease = {}
      inboundWatchdogLease = watchdogLease
      inboundWatchdogTimer = scheduler.setTimeout(() => {
        if (
          inboundWatchdogLease !== watchdogLease
          || !isCurrentConnection()
        ) return
        inboundWatchdogTimer = null
        inboundWatchdogLease = null
        disconnect(true, 'stale')
      }, inboundIdleTimeoutMs)
    }

    next.addEventListener('open', () => {
      if (!isCurrentConnection()) return
      reconnectAttempt = 0
      clearHeartbeat()
      const timerLease = connectionLease
      heartbeatTimerLease = timerLease
      heartbeatTimer = scheduler.setInterval(() => {
        if (
          heartbeatTimerLease !== timerLease
          || !isCurrentConnection()
          || next.readyState !== SOCKET_OPEN
        ) return
        try {
          next.send('ping')
        } catch {
          disconnect(true, 'connecting')
        }
      }, heartbeatMs)
      armInboundWatchdog()
      notifyState('live')
      try {
        options.onOpen?.()
      } catch {
        // Consumer callbacks do not own the transport lifecycle.
      }
    })

    next.addEventListener('message', (event) => {
      if (!isCurrentConnection()) return
      // Every inbound frame proves the reverse path, even when the frame is a
      // pong, malformed, unknown, or otherwise not a business event.
      armInboundWatchdog()
      try {
        options.onInboundFrame?.(safeNow(now))
      } catch {
        // Diagnostics cannot disconnect the private stream.
      }
      notifyState('live')
      const frame = parsePrivateUserFrame(event.data)
      if (!frame) return
      try {
        options.onEvent(frame)
      } catch {
        // A consumer event handler must not disconnect the private session.
      }
    })

    next.addEventListener('close', () => disconnect(false, 'connecting'))
    next.addEventListener('error', () => disconnect(true, 'connecting'))
  }

  return {
    start(): boolean {
      if (running) return true
      const accessToken = readLatestAccessToken()
      if (!accessToken) return false
      running = true
      reconnectAttempt = 0
      const lease = {}
      lifecycleLease = lease
      notifyState('connecting')
      connect(lease)
      return running
    },
    stop(): void {
      if (
        !running
        && lifecycleLease === null
        && socket === null
        && reconnectTimer === null
        && heartbeatTimer === null
        && inboundWatchdogTimer === null
      ) return
      running = false
      lifecycleLease = null
      reconnectAttempt = 0
      clearReconnect()
      clearHeartbeat()
      clearInboundWatchdog()
      const current = socket
      socket = null
      notifyState('stopped')
      if (!current) return
      try {
        current.close()
      } catch {
        return
      }
    },
    isRunning: () => running,
  }
}

function positiveDelay(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : fallback
}

function normalizedRatio(value: number | undefined, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback
  return Math.min(1, Math.max(0, value))
}

function safeRandom(random: () => number): number {
  try {
    const value = random()
    return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0.5
  } catch {
    return 0.5
  }
}

function jitteredDelay(
  baseDelay: number,
  maximumDelay: number,
  ratio: number,
  random: number,
): number {
  const factor = 1 - ratio + (2 * ratio * random)
  return Math.max(1, Math.min(maximumDelay, Math.round(baseDelay * factor)))
}

function safeNow(now: () => number): number {
  try {
    const value = now()
    return Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0
  } catch {
    return 0
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
