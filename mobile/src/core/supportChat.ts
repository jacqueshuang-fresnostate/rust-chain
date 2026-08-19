import type { Message } from '../api/support.ts'

export const SUPPORT_RECONCILE_INTERVAL_MS = 5_000
export const SUPPORT_MESSAGE_MAX_SCALARS = 2_000

export type SupportChatViewState = 'guest' | 'loading' | 'error' | 'empty' | 'ready'

export interface SupportSendAttempt {
  readonly body: string
  readonly clientMessageId: string
}

export interface SupportMessageGroup {
  readonly dayKey: string
  readonly firstCreatedAt: number
  readonly messages: readonly Message[]
}

export interface SupportHistoryPage {
  readonly messages: readonly Message[]
  readonly has_more: boolean
  readonly next_before_id: number | null
}

export interface SupportHistoryMergeResult {
  readonly messages: readonly Message[]
  readonly hasMore: boolean
  readonly nextBeforeId: number | null
}

export interface SupportPollingScheduler {
  setInterval(callback: () => void, delay: number): unknown
  clearInterval(handle: unknown): void
}

export interface SupportPollingController {
  isRunning(): boolean
  start(): void
  stop(): void
}

const defaultPollingScheduler: SupportPollingScheduler = {
  setInterval: (callback, delay) => globalThis.setInterval(callback, delay),
  clearInterval: (handle) => globalThis.clearInterval(handle as ReturnType<typeof globalThis.setInterval>),
}

export function supportMessageScalarLength(value: string): number {
  return Array.from(value).length
}

export function createSupportClientMessageId(
  now = Date.now(),
  entropy = createSupportEntropy(),
): string {
  const safeEntropy = entropy.replace(/[^A-Za-z0-9_-]/g, '').slice(0, 36) || 'local'
  return `mobile-${Math.max(0, Math.trunc(now)).toString(36)}-${safeEntropy}`.slice(0, 64)
}

export function createSupportSendAttempt(
  body: string,
  createClientMessageId: () => string = () => createSupportClientMessageId(),
): SupportSendAttempt {
  const normalizedBody = body.trim()
  const scalarLength = supportMessageScalarLength(normalizedBody)
  if (!normalizedBody || scalarLength > SUPPORT_MESSAGE_MAX_SCALARS) {
    throw new RangeError('Support message body is outside the accepted boundary')
  }
  const clientMessageId = createClientMessageId()
  if (!/^[A-Za-z0-9_-]{8,64}$/.test(clientMessageId)) {
    throw new RangeError('Support client message id is outside the accepted boundary')
  }
  return Object.freeze({ body: normalizedBody, clientMessageId })
}

export function executeSupportSendAttempt<T>(
  attempt: SupportSendAttempt,
  send: (body: string, clientMessageId: string) => Promise<T>,
): Promise<T> {
  return send(attempt.body, attempt.clientMessageId)
}

export function reconcileSupportMessages(
  current: readonly Message[],
  incoming: readonly Message[],
): readonly Message[] {
  const messagesById = new Map<number, Message>()
  for (const message of current) messagesById.set(message.id, message)
  for (const message of incoming) messagesById.set(message.id, message)
  return [...messagesById.values()].sort((left, right) => (
    left.created_at - right.created_at || left.id - right.id
  ))
}

export function mergeSupportHistoryPage(
  current: readonly Message[],
  page: SupportHistoryPage,
): SupportHistoryMergeResult {
  const nextBeforeId = page.has_more ? page.next_before_id : null
  return Object.freeze({
    messages: Object.freeze(reconcileSupportMessages(current, page.messages)),
    hasMore: page.has_more && nextBeforeId !== null,
    nextBeforeId,
  })
}

export function groupSupportMessages(messages: readonly Message[]): readonly SupportMessageGroup[] {
  const groups: SupportMessageGroup[] = []
  for (const message of reconcileSupportMessages([], messages)) {
    const dayKey = supportMessageDayKey(message.created_at)
    const currentGroup = groups.at(-1)
    if (currentGroup?.dayKey === dayKey) {
      groups[groups.length - 1] = Object.freeze({
        ...currentGroup,
        messages: Object.freeze([...currentGroup.messages, message]),
      })
      continue
    }
    groups.push(Object.freeze({
      dayKey,
      firstCreatedAt: message.created_at,
      messages: Object.freeze([message]),
    }))
  }
  return Object.freeze(groups)
}

export function supportMessageDayKey(timestamp: number): string {
  const date = new Date(timestamp)
  if (!Number.isFinite(timestamp) || Number.isNaN(date.getTime())) return 'invalid'
  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, '0'),
    String(date.getDate()).padStart(2, '0'),
  ].join('-')
}

export function latestRenderedStaffMessageId(messages: readonly Message[]): number | null {
  let latestId: number | null = null
  for (const message of messages) {
    if (message.sender_type !== 'agent' && message.sender_type !== 'admin') continue
    latestId = latestId === null ? message.id : Math.max(latestId, message.id)
  }
  return latestId
}

export function resolveSupportChatViewState(input: {
  authenticated: boolean
  loading: boolean
  failed: boolean
  messageCount: number
}): SupportChatViewState {
  if (!input.authenticated) return 'guest'
  if (input.loading) return 'loading'
  if (input.failed) return 'error'
  return input.messageCount > 0 ? 'ready' : 'empty'
}

export function createSupportPollingController(
  refresh: () => Promise<void> | void,
  intervalMs = SUPPORT_RECONCILE_INTERVAL_MS,
  scheduler: SupportPollingScheduler = defaultPollingScheduler,
): SupportPollingController {
  let active = false
  let inFlight = false
  let handle: unknown = null

  async function tick(): Promise<void> {
    if (!active || inFlight) return
    inFlight = true
    try {
      await refresh()
    } catch {
      // Refresh owns its visible error state; the interval remains recoverable.
    } finally {
      inFlight = false
    }
  }

  return {
    isRunning: () => active,
    start() {
      if (active) return
      active = true
      handle = scheduler.setInterval(() => { void tick() }, intervalMs)
    },
    stop() {
      if (!active) return
      active = false
      if (handle !== null) scheduler.clearInterval(handle)
      handle = null
    },
  }
}

function createSupportEntropy(): string {
  const cryptoApi = globalThis.crypto
  if (cryptoApi && typeof cryptoApi.randomUUID === 'function') {
    return cryptoApi.randomUUID().replace(/-/g, '')
  }
  return `${Math.random().toString(36).slice(2)}${Math.random().toString(36).slice(2)}`
}
