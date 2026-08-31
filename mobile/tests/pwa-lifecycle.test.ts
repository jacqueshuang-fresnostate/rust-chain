import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createPwaInstallEligibilitySession,
  isPwaInstallValueRoute,
  PWA_INSTALL_FREQUENCY_CAP_MS,
  PWA_INSTALL_SESSION_DELAY_MS,
} from '../src/pwa/eligibility.ts'
import {
  runPwaUpdate,
  type PwaUpdateFailureReason,
  type PwaUpdateTimer,
} from '../src/pwa/update.ts'

class FakeTimer implements PwaUpdateTimer {
  private elapsed = 0
  private nextId = 1
  private readonly tasks = new Map<number, { at: number; callback: () => void }>()

  clear(handle: unknown): void {
    this.tasks.delete(Number(handle))
  }

  set(callback: () => void, delayMs: number): unknown {
    const id = this.nextId++
    this.tasks.set(id, { at: this.elapsed + delayMs, callback })
    return id
  }

  advance(delayMs: number): void {
    const target = this.elapsed + delayMs
    while (true) {
      const next = [...this.tasks.entries()]
        .filter(([, task]) => task.at <= target)
        .sort((left, right) => left[1].at - right[1].at || left[0] - right[0])[0]
      if (!next) break
      this.tasks.delete(next[0])
      this.elapsed = next[1].at
      next[1].callback()
    }
    this.elapsed = target
  }
}

class FakeWorker extends EventTarget {
  state: ServiceWorkerState = 'installed'
  readonly messages: unknown[] = []
  onPostMessage?: () => void

  postMessage(message: unknown): void {
    this.messages.push(message)
    this.onPostMessage?.()
  }
}

class FakeRegistration extends EventTarget {
  installing: FakeWorker | null = null
  waiting: FakeWorker | null
  readonly update: () => Promise<ServiceWorkerRegistration>

  constructor(waiting: FakeWorker | null, update?: () => Promise<ServiceWorkerRegistration>) {
    super()
    this.waiting = waiting
    this.update = update || (async () => this as unknown as ServiceWorkerRegistration)
  }
}

function updateHarness(options: {
  controller: EventTarget
  timer: FakeTimer
  waiting: FakeWorker | null
  update?: () => Promise<ServiceWorkerRegistration>
}) {
  const busy: boolean[] = []
  const errors: boolean[] = []
  const failures: PwaUpdateFailureReason[] = []
  let reloads = 0

  const registration = new FakeRegistration(options.waiting, options.update)
  const run = () => runPwaUpdate({
    registration: registration as unknown as ServiceWorkerRegistration,
    controllerTarget: options.controller as unknown as ServiceWorkerContainer,
    timeoutMs: 100,
    timer: options.timer,
    onBusyChange: (value) => busy.push(value),
    onErrorChange: (value) => errors.push(value),
    onFailure: (reason) => failures.push(reason),
    reload: () => { reloads += 1 },
  })

  return { busy, errors, failures, registration, reloads: () => reloads, run }
}

test('PWA update waits for controllerchange, clears busy, and reloads once', async () => {
  const controller = new EventTarget()
  const timer = new FakeTimer()
  const worker = new FakeWorker()
  worker.onPostMessage = () => controller.dispatchEvent(new Event('controllerchange'))
  const harness = updateHarness({ controller, timer, waiting: worker })

  assert.equal(await harness.run(), true)
  assert.deepEqual(worker.messages, [{ type: 'SKIP_WAITING' }])
  assert.deepEqual(harness.busy, [true, false])
  assert.deepEqual(harness.errors, [false])
  assert.deepEqual(harness.failures, [])
  assert.equal(harness.reloads(), 1)
})

test('PWA update times out without controllerchange, recovers busy, and retries successfully', async () => {
  const controller = new EventTarget()
  const timer = new FakeTimer()
  const worker = new FakeWorker()
  const harness = updateHarness({ controller, timer, waiting: worker })

  const timedOut = harness.run()
  assert.deepEqual(harness.busy, [true])
  timer.advance(100)
  assert.equal(await timedOut, false)
  assert.deepEqual(harness.busy, [true, false])
  assert.deepEqual(harness.errors, [false, true])
  assert.deepEqual(harness.failures, ['activation-timeout'])
  assert.equal(harness.reloads(), 0)

  worker.onPostMessage = () => controller.dispatchEvent(new Event('controllerchange'))
  assert.equal(await harness.run(), true)
  assert.equal(worker.messages.length, 2)
  assert.deepEqual(harness.busy, [true, false, true, false])
  assert.deepEqual(harness.errors, [false, true, false])
  assert.equal(harness.reloads(), 1)
})

test('PWA update check has its own deadline and never leaves busy set', async () => {
  const controller = new EventTarget()
  const timer = new FakeTimer()
  const harness = updateHarness({
    controller,
    timer,
    waiting: null,
    update: () => new Promise<ServiceWorkerRegistration>(() => {}),
  })

  const result = harness.run()
  timer.advance(100)
  assert.equal(await result, false)
  assert.deepEqual(harness.busy, [true, false])
  assert.deepEqual(harness.failures, ['update-check-timeout'])
})

test('PWA update recovers when the waiting worker rejects activation or becomes redundant', async () => {
  const throwingController = new EventTarget()
  const throwingTimer = new FakeTimer()
  const throwingWorker = new FakeWorker()
  throwingWorker.onPostMessage = () => { throw new Error('fixture postMessage failure') }
  const throwingHarness = updateHarness({
    controller: throwingController,
    timer: throwingTimer,
    waiting: throwingWorker,
  })

  assert.equal(await throwingHarness.run(), false)
  assert.deepEqual(throwingHarness.busy, [true, false])
  assert.deepEqual(throwingHarness.failures, ['post-message-failed'])

  const redundantController = new EventTarget()
  const redundantTimer = new FakeTimer()
  const redundantWorker = new FakeWorker()
  redundantWorker.onPostMessage = () => {
    redundantWorker.state = 'redundant'
    redundantWorker.dispatchEvent(new Event('statechange'))
  }
  const redundantHarness = updateHarness({
    controller: redundantController,
    timer: redundantTimer,
    waiting: redundantWorker,
  })

  assert.equal(await redundantHarness.run(), false)
  assert.deepEqual(redundantHarness.busy, [true, false])
  assert.deepEqual(redundantHarness.failures, ['worker-redundant'])
})

test('PWA update reports a recoverable failure when an update check yields no waiting worker', async () => {
  const harness = updateHarness({
    controller: new EventTarget(),
    timer: new FakeTimer(),
    waiting: null,
  })

  assert.equal(await harness.run(), false)
  assert.deepEqual(harness.busy, [true, false])
  assert.deepEqual(harness.errors, [false, true])
  assert.deepEqual(harness.failures, ['no-waiting-worker'])
})

test('PWA update waits for a newly installing worker before requesting activation', async () => {
  const controller = new EventTarget()
  const timer = new FakeTimer()
  const worker = new FakeWorker()
  worker.state = 'installing'
  let registration: FakeRegistration
  const harness = updateHarness({
    controller,
    timer,
    waiting: null,
    update: async () => {
      registration.installing = worker
      registration.dispatchEvent(new Event('updatefound'))
      return registration as unknown as ServiceWorkerRegistration
    },
  })
  registration = harness.registration
  worker.onPostMessage = () => controller.dispatchEvent(new Event('controllerchange'))

  const result = harness.run()
  await Promise.resolve()
  worker.state = 'installed'
  worker.dispatchEvent(new Event('statechange'))
  assert.equal(await result, true)
  assert.deepEqual(worker.messages, [{ type: 'SKIP_WAITING' }])
  assert.deepEqual(harness.failures, [])
})

test('install eligibility requires session delay and a value action, then caps a closed offer', () => {
  const startedAt = 1_000
  const session = createPwaInstallEligibilitySession(startedAt)
  const base = {
    hasInstallSurface: true,
    isStandalone: false,
  }

  assert.deepEqual(session.evaluate({ ...base, now: startedAt + PWA_INSTALL_SESSION_DELAY_MS + 1 }), {
    eligible: false,
    newlyGranted: false,
  })

  session.recordValueAction()
  assert.deepEqual(session.evaluate({ ...base, now: startedAt + PWA_INSTALL_SESSION_DELAY_MS - 1 }), {
    eligible: false,
    newlyGranted: false,
  })

  const shownAt = startedAt + PWA_INSTALL_SESSION_DELAY_MS
  assert.deepEqual(session.evaluate({ ...base, now: shownAt }), {
    eligible: true,
    newlyGranted: true,
  })
  assert.equal(session.markOfferShown(), true)
  assert.equal(session.markOfferShown(), false)
  assert.deepEqual(session.evaluate({ ...base, now: shownAt + 1, lastShownAt: shownAt }), {
    eligible: true,
    newlyGranted: false,
  })

  session.closeOffer()
  assert.deepEqual(session.evaluate({ ...base, now: shownAt + 1, lastShownAt: shownAt }), {
    eligible: false,
    newlyGranted: false,
  })
  assert.deepEqual(session.evaluate({
    ...base,
    now: shownAt + PWA_INSTALL_FREQUENCY_CAP_MS,
    lastShownAt: shownAt,
  }), {
    eligible: true,
    newlyGranted: true,
  })
})

test('install value actions are explicit business routes rather than every route visit', () => {
  for (const route of ['market-detail', 'trade', 'seconds', 'assets', 'wallet-ledger']) {
    assert.equal(isPwaInstallValueRoute(route), true, route)
  }
  for (const route of ['home', 'markets', 'login', 'register', undefined]) {
    assert.equal(isPwaInstallValueRoute(route), false, String(route))
  }
})
