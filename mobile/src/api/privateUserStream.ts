const SOCKET_OPEN = 1

export const PRIVATE_USER_RECONNECT_BASE_MS = 1_000
export const PRIVATE_USER_RECONNECT_MAX_MS = 30_000
export const PRIVATE_USER_HEARTBEAT_MS = 25_000

const IGNORED_FRAME_TYPES = new Set([
  'pong',
  'subscribed',
  'unsubscribed',
  'subscription.confirmed',
  'subscription_confirmation',
  'error',
])

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
  onEvent(event: PrivateUserEvent): void
  createSocket?: (url: string) => PrivateUserSocket | null
  scheduler?: PrivateUserStreamScheduler
  reconnectBaseMs?: number
  reconnectMaxMs?: number
  heartbeatMs?: number
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
 * Owns one authenticated private-user socket. The server binds the user
 * channel during the handshake, so the client sends heartbeat frames only.
 */
export function createPrivateUserStream(
  options: PrivateUserStreamOptions,
): PrivateUserStreamSession {
  const scheduler = options.scheduler ?? defaultScheduler
  const createSocket = options.createSocket ?? createBrowserSocket
  const reconnectBaseMs = positiveDelay(
    options.reconnectBaseMs,
    PRIVATE_USER_RECONNECT_BASE_MS,
  )
  const reconnectMaxMs = Math.max(
    reconnectBaseMs,
    positiveDelay(options.reconnectMaxMs, PRIVATE_USER_RECONNECT_MAX_MS),
  )
  const heartbeatMs = positiveDelay(options.heartbeatMs, PRIVATE_USER_HEARTBEAT_MS)

  let running = false
  let lifecycleLease: object | null = null
  let socket: PrivateUserSocket | null = null
  let reconnectAttempt = 0
  let reconnectTimer: unknown = null
  let reconnectTimerLease: object | null = null
  let heartbeatTimer: unknown = null
  let heartbeatTimerLease: object | null = null

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

  const deactivateForMissingToken = (lease: object): void => {
    if (lifecycleLease !== lease) return
    running = false
    lifecycleLease = null
    clearReconnect()
    clearHeartbeat()
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

    const delay = Math.min(reconnectBaseMs * 2 ** reconnectAttempt, reconnectMaxMs)
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

    const disconnect = (closeSocket: boolean): void => {
      if (!isCurrentConnection()) return
      disconnected = true
      socket = null
      clearHeartbeat()
      if (closeSocket) {
        try {
          next.close()
        } catch {
          // Reconnect scheduling remains authoritative after a failed close.
        }
      }
      scheduleReconnect(lease)
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
          disconnect(true)
        }
      }, heartbeatMs)
      try {
        options.onOpen?.()
      } catch {
        // View callbacks do not own the transport lifecycle.
      }
    })

    next.addEventListener('message', (event) => {
      if (!isCurrentConnection()) return
      const frame = parsePrivateUserFrame(event.data)
      if (!frame) return
      try {
        options.onEvent(frame)
      } catch {
        // A view event handler must not disconnect the private session.
      }
    })

    next.addEventListener('close', () => disconnect(false))
    next.addEventListener('error', () => disconnect(true))
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
      ) return
      running = false
      lifecycleLease = null
      reconnectAttempt = 0
      clearReconnect()
      clearHeartbeat()
      const current = socket
      socket = null
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
