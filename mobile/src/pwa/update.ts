export const PWA_UPDATE_TIMEOUT_MS = 15_000

export type PwaUpdateFailureReason =
  | 'activation-timeout'
  | 'no-waiting-worker'
  | 'post-message-failed'
  | 'update-check-failed'
  | 'update-check-timeout'
  | 'worker-redundant'

export interface PwaUpdateTimer {
  clear(handle: unknown): void
  set(callback: () => void, delayMs: number): unknown
}

export interface RunPwaUpdateOptions {
  controllerTarget: Pick<ServiceWorkerContainer, 'addEventListener' | 'removeEventListener'>
  onBusyChange(busy: boolean): void
  onErrorChange(error: boolean): void
  onFailure?(reason: PwaUpdateFailureReason): void
  registration: Pick<ServiceWorkerRegistration,
    'addEventListener' | 'installing' | 'removeEventListener' | 'update' | 'waiting'>
  reload(): void
  timeoutMs?: number
  timer?: PwaUpdateTimer
}

function observeInstallingWorker(
  registration: RunPwaUpdateOptions['registration'],
): { cleanup(): void; current(): ServiceWorker | null } {
  let worker = registration.installing
  const onUpdateFound: EventListener = () => {
    worker = registration.installing || worker
  }
  registration.addEventListener('updatefound', onUpdateFound)
  return {
    cleanup: () => registration.removeEventListener('updatefound', onUpdateFound),
    current: () => registration.waiting || registration.installing || worker,
  }
}

function waitForWorkerInstalled(
  worker: ServiceWorker,
  timeoutMs: number,
  timer: PwaUpdateTimer,
): Promise<ServiceWorker> {
  if (worker.state === 'installed' || worker.state === 'activating' || worker.state === 'activated') {
    return Promise.resolve(worker)
  }
  if (worker.state === 'redundant') return Promise.reject(new PwaUpdateFailure('worker-redundant'))
  return new Promise<ServiceWorker>((resolve, reject) => {
    let settled = false
    let timeout: unknown
    const cleanup = () => {
      worker.removeEventListener('statechange', onStateChange)
      timer.clear(timeout)
    }
    const finish = (failure?: PwaUpdateFailureReason) => {
      if (settled) return
      settled = true
      cleanup()
      if (failure) reject(new PwaUpdateFailure(failure))
      else resolve(worker)
    }
    const onStateChange: EventListener = () => {
      if (worker.state === 'redundant') finish('worker-redundant')
      else if (worker.state === 'installed' || worker.state === 'activating' || worker.state === 'activated') finish()
    }
    worker.addEventListener('statechange', onStateChange)
    timeout = timer.set(() => finish('no-waiting-worker'), timeoutMs)
    onStateChange(new Event('statechange'))
  })
}

class PwaUpdateFailure extends Error {
  readonly reason: PwaUpdateFailureReason

  constructor(reason: PwaUpdateFailureReason) {
    super(reason)
    this.name = 'PwaUpdateFailure'
    this.reason = reason
  }
}

const browserTimer: PwaUpdateTimer = {
  clear(handle) {
    globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>)
  },
  set(callback, delayMs) {
    return globalThis.setTimeout(callback, delayMs)
  },
}

function withinDeadline<T>(
  promise: Promise<T>,
  timeoutMs: number,
  timer: PwaUpdateTimer,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let settled = false
    const timeout = timer.set(() => {
      if (settled) return
      settled = true
      reject(new PwaUpdateFailure('update-check-timeout'))
    }, timeoutMs)

    promise.then(
      (value) => {
        if (settled) return
        settled = true
        timer.clear(timeout)
        resolve(value)
      },
      () => {
        if (settled) return
        settled = true
        timer.clear(timeout)
        reject(new PwaUpdateFailure('update-check-failed'))
      },
    )
  })
}

function waitForControllerChange(
  worker: ServiceWorker,
  controllerTarget: Pick<ServiceWorkerContainer, 'addEventListener' | 'removeEventListener'>,
  timeoutMs: number,
  timer: PwaUpdateTimer,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    let settled = false
    let timeout: unknown

    const cleanup = () => {
      controllerTarget.removeEventListener('controllerchange', onControllerChange)
      worker.removeEventListener('statechange', onWorkerStateChange)
      if (timeout !== undefined) timer.clear(timeout)
    }
    const succeed = () => {
      if (settled) return
      settled = true
      cleanup()
      resolve()
    }
    const fail = (reason: PwaUpdateFailureReason) => {
      if (settled) return
      settled = true
      cleanup()
      reject(new PwaUpdateFailure(reason))
    }
    const onControllerChange: EventListener = () => succeed()
    const onWorkerStateChange: EventListener = () => {
      if (worker.state === 'redundant') fail('worker-redundant')
    }

    if (worker.state === 'redundant') {
      fail('worker-redundant')
      return
    }

    controllerTarget.addEventListener('controllerchange', onControllerChange)
    worker.addEventListener('statechange', onWorkerStateChange)
    timeout = timer.set(() => fail('activation-timeout'), timeoutMs)

    try {
      worker.postMessage({ type: 'SKIP_WAITING' })
    } catch {
      fail('post-message-failed')
    }
  })
}

function failureReason(error: unknown): PwaUpdateFailureReason {
  return error instanceof PwaUpdateFailure ? error.reason : 'update-check-failed'
}

export async function runPwaUpdate(options: RunPwaUpdateOptions): Promise<boolean> {
  const timeoutMs = Number.isFinite(options.timeoutMs) && Number(options.timeoutMs) > 0
    ? Number(options.timeoutMs)
    : PWA_UPDATE_TIMEOUT_MS
  const timer = options.timer || browserTimer

  options.onBusyChange(true)
  options.onErrorChange(false)

  try {
    let waitingWorker = options.registration.waiting
    if (!waitingWorker) {
      const installing = observeInstallingWorker(options.registration)
      try {
        await withinDeadline(options.registration.update(), timeoutMs, timer)
        waitingWorker = options.registration.waiting
        const candidate = waitingWorker || installing.current()
        if (candidate) waitingWorker = await waitForWorkerInstalled(candidate, timeoutMs, timer)
      } finally {
        installing.cleanup()
      }
    }
    if (!waitingWorker) throw new PwaUpdateFailure('no-waiting-worker')

    await waitForControllerChange(waitingWorker, options.controllerTarget, timeoutMs, timer)
    options.reload()
    return true
  } catch (error) {
    const reason = failureReason(error)
    options.onErrorChange(true)
    options.onFailure?.(reason)
    return false
  } finally {
    options.onBusyChange(false)
  }
}
