import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createMarginAccountReconciliationLifecycle,
  MARGIN_ACCOUNT_RECONCILIATION_INTERVAL_MS,
  reconcileMarginRiskSnapshots,
  type MarginAccountReconciliationRequest,
  type MarginAccountReconciliationScheduler,
} from '../src/core/marginAccountReconciliation.ts'

const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
const lifecycleSource = readFileSync(
  new URL('../src/core/marginAccountReconciliation.ts', import.meta.url),
  'utf8',
)

test('杠杆账户后台对账单飞执行并按访客、现货和隐藏状态跳过周期请求', async () => {
  let sessionKey = ''
  let contractMode = false
  let visible = true
  let reconcileCount = 0
  const scheduled = { callback: null as (() => void) | null }
  const cleared: unknown[] = []
  const scheduler: MarginAccountReconciliationScheduler = {
    setInterval(callback, delay) {
      assert.equal(delay, MARGIN_ACCOUNT_RECONCILIATION_INTERVAL_MS)
      scheduled.callback = callback
      return 41
    },
    clearInterval(handle) {
      cleared.push(handle)
      scheduled.callback = null
    },
  }
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => sessionKey,
    isContractMode: () => contractMode,
    isVisible: () => visible,
    scheduler,
    reconcile: async () => {
      reconcileCount += 1
    },
  })

  lifecycle.startPolling()
  lifecycle.startPolling()
  assert.ok(scheduled.callback)

  scheduled.callback?.()
  await tick()
  assert.equal(reconcileCount, 0)

  sessionKey = 'TOKEN_A'
  scheduled.callback?.()
  await tick()
  assert.equal(reconcileCount, 0)

  contractMode = true
  visible = false
  scheduled.callback?.()
  await tick()
  assert.equal(reconcileCount, 0)

  visible = true
  const gate = deferred<void>()
  const singleFlight = createMarginAccountReconciliationLifecycle({
    sessionKey: () => sessionKey,
    isContractMode: () => contractMode,
    isVisible: () => visible,
    reconcile: async () => {
      reconcileCount += 1
      await gate.promise
    },
  })
  const first = singleFlight.refreshBackground()
  assert.equal(singleFlight.isBackgroundInFlight(), true)
  assert.deepEqual(await singleFlight.refreshBackground(), {
    state: 'skipped',
    reason: 'single-flight',
  })
  gate.resolve(undefined)
  assert.deepEqual(await first, { state: 'completed' })
  assert.equal(singleFlight.isBackgroundInFlight(), false)

  const restoreGates: Array<ReturnType<typeof deferred<void>>> = []
  const restoreCommits: number[] = []
  const visibilityLifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => sessionKey,
    isContractMode: () => contractMode,
    isVisible: () => visible,
    reconcile: async (request) => {
      const requestNumber = restoreGates.length + 1
      const requestGate = deferred<void>()
      restoreGates.push(requestGate)
      await requestGate.promise
      request.commit(() => restoreCommits.push(requestNumber))
    },
  })
  const beforeHide = visibilityLifecycle.refreshBackground()
  visible = false
  visibilityLifecycle.invalidate()
  visible = true
  assert.deepEqual(await visibilityLifecycle.refreshBackground(), {
    state: 'skipped',
    reason: 'single-flight',
  })
  restoreGates[0]?.resolve(undefined)
  assert.deepEqual(await beforeHide, { state: 'stale' })
  await tick()
  assert.equal(restoreGates.length, 2, 'visibility restore queues one fresh non-overlapping poll')
  restoreGates[1]?.resolve(undefined)
  await tick()
  assert.deepEqual(restoreCommits, [2])
  visibilityLifecycle.stop()

  lifecycle.stop()
  assert.deepEqual(cleared, [41])
  assert.equal(lifecycle.isPolling(), false)
})

test('代次守卫隔离模式 ABA、token 切换、卸载和前台刷新覆盖旧轮询', async () => {
  let sessionKey = 'TOKEN_A'
  let contractMode = true
  const commits: string[] = []
  const requests: Array<{
    request: MarginAccountReconciliationRequest
    sessionKey: string
    gate: ReturnType<typeof deferred<void>>
  }> = []
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => sessionKey,
    isContractMode: () => contractMode,
    isVisible: () => true,
    reconcile: async (request) => {
      const gate = deferred<void>()
      const requestSessionKey = sessionKey
      requests.push({ request, sessionKey: requestSessionKey, gate })
      await gate.promise
      request.commit(() => commits.push(`${request.kind}:${requestSessionKey}`))
    },
  })

  const oldPoll = lifecycle.refreshBackground()
  contractMode = false
  lifecycle.invalidate()
  contractMode = true
  lifecycle.invalidate()
  const foreground = lifecycle.refreshForeground()
  assert.equal(requests.length, 2)
  assert.equal(requests[0]?.request.isCurrent(), false)
  assert.equal(requests[1]?.request.isCurrent(), true)

  requests[0]?.gate.resolve(undefined)
  assert.deepEqual(await oldPoll, { state: 'stale' })
  assert.deepEqual(commits, [])
  requests[1]?.gate.resolve(undefined)
  assert.deepEqual(await foreground, { state: 'completed' })
  assert.deepEqual(commits, ['foreground:TOKEN_A'])

  const beforeTokenSwitch = lifecycle.refreshBackground()
  sessionKey = 'TOKEN_B'
  lifecycle.invalidate()
  requests[2]?.gate.resolve(undefined)
  assert.deepEqual(await beforeTokenSwitch, { state: 'stale' })
  assert.deepEqual(commits, ['foreground:TOKEN_A'])

  const beforeUnmount = lifecycle.refreshForeground()
  lifecycle.stop()
  requests[3]?.gate.resolve(undefined)
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(commits, ['foreground:TOKEN_A'])
  assert.deepEqual(await lifecycle.refreshForeground(), {
    state: 'skipped',
    reason: 'inactive',
  })
})

test('后台失败保留最后成功状态且下一轮可自动恢复', async () => {
  let attempt = 0
  let renderedState = 'last-success'
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => 'TOKEN_A',
    isContractMode: () => true,
    isVisible: () => true,
    reconcile: async (request) => {
      attempt += 1
      if (attempt === 1) throw new Error('transient')
      request.commit(() => {
        renderedState = 'recovered'
      })
    },
  })

  const failed = await lifecycle.refreshBackground()
  assert.equal(failed.state, 'error')
  assert.equal(renderedState, 'last-success')
  assert.equal(lifecycle.isBackgroundInFlight(), false)

  assert.deepEqual(await lifecycle.refreshBackground(), { state: 'completed' })
  assert.equal(renderedState, 'recovered')
})

test('私有事件提示在请求忙时合并为一次后续权威对账', async () => {
  const gates: Array<ReturnType<typeof deferred<void>>> = []
  const commits: string[] = []
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => 'TOKEN_A',
    isContractMode: () => true,
    isVisible: () => true,
    reconcile: async (request) => {
      const gate = deferred<void>()
      const label = `${request.kind}:${gates.length + 1}`
      gates.push(gate)
      await gate.promise
      request.commit(() => commits.push(label))
    },
  })

  const poll = lifecycle.refreshBackground()
  assert.deepEqual(await lifecycle.refreshBackground({ queueIfBusy: true }), {
    state: 'skipped',
    reason: 'single-flight',
  })
  assert.deepEqual(await lifecycle.refreshBackground({ queueIfBusy: true }), {
    state: 'skipped',
    reason: 'single-flight',
  })
  gates[0]?.resolve(undefined)
  assert.deepEqual(await poll, { state: 'stale' })
  await tick()
  assert.equal(gates.length, 2, 'repeated hints supersede the old poll and coalesce into one follow-up')
  gates[1]?.resolve(undefined)
  await tick()

  const foreground = lifecycle.refreshForeground()
  assert.deepEqual(await lifecycle.refreshBackground({ queueIfBusy: true }), {
    state: 'skipped',
    reason: 'foreground',
  })
  gates[2]?.resolve(undefined)
  assert.deepEqual(await foreground, { state: 'completed' })
  await tick()
  assert.equal(gates.length, 4, 'a hint queued behind a mutation refresh runs after it')
  gates[3]?.resolve(undefined)
  await tick()

  assert.deepEqual(commits, [
    'background:2',
    'foreground:3',
    'background:4',
  ])
  lifecycle.stop()
})

test('foreground supersedes an old poll while loading, then queued refresh starts after foreground cleanup', async () => {
  const gates: Array<ReturnType<typeof deferred<void>>> = []
  const commits: string[] = []
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => 'TOKEN_A',
    isContractMode: () => true,
    isVisible: () => true,
    reconcile: async (request) => {
      const gate = deferred<void>()
      const label = `${request.kind}:${gates.length + 1}`
      gates.push(gate)
      await gate.promise
      request.commit(() => commits.push(label))
    },
  })

  const oldPoll = lifecycle.refreshBackground()
  let balancesLoading = true
  const foreground = lifecycle.refreshForeground().finally(() => {
    balancesLoading = false
  })
  assert.equal(gates.length, 2)
  assert.equal(balancesLoading, true)

  assert.deepEqual(await lifecycle.refreshBackground({ queueIfBusy: true }), {
    state: 'skipped',
    reason: 'foreground',
  })
  gates[1]?.resolve(undefined)
  assert.deepEqual(await foreground, { state: 'completed' })
  assert.equal(balancesLoading, false, 'foreground owns loading until its caller settles')
  assert.equal(gates.length, 2, 'queued background work cannot race foreground cleanup')

  gates[0]?.resolve(undefined)
  assert.deepEqual(await oldPoll, { state: 'stale' })
  assert.equal(gates.length, 2)
  await tick()
  assert.equal(gates.length, 3, 'one queued refresh starts after both older requests settle')
  gates[2]?.resolve(undefined)
  await tick()

  assert.deepEqual(commits, ['foreground:2', 'background:3'])
  lifecycle.stop()
})

test('queued background recovery cannot finish before foreground error and loading state settle', async () => {
  const gates: Array<ReturnType<typeof deferred<void>>> = []
  const order: string[] = []
  let balancesLoading = false
  let balancesError = false
  const lifecycle = createMarginAccountReconciliationLifecycle({
    sessionKey: () => 'TOKEN_A',
    isContractMode: () => true,
    isVisible: () => true,
    reconcile: async (request) => {
      order.push(`${request.kind}-start`)
      const gate = deferred<void>()
      gates.push(gate)
      await gate.promise
      if (request.kind === 'background') {
        request.commit(() => {
          balancesError = false
        })
      }
    },
  })

  balancesLoading = true
  const foreground = (async () => {
    const result = await lifecycle.refreshForeground()
    if (result.state === 'error') balancesError = true
    balancesLoading = false
    order.push('foreground-ui-settled')
    return result
  })()
  assert.deepEqual(await lifecycle.refreshBackground({ queueIfBusy: true }), {
    state: 'skipped',
    reason: 'foreground',
  })

  gates[0]?.reject(new Error('foreground failed'))
  assert.equal((await foreground).state, 'error')
  assert.equal(balancesLoading, false)
  assert.equal(balancesError, true)
  assert.deepEqual(order.slice(0, 3), [
    'foreground-start',
    'foreground-ui-settled',
    'background-start',
  ], 'queued recovery starts only after foreground UI cleanup')

  await tick()
  assert.equal(gates.length, 2)
  gates[1]?.resolve(undefined)
  await tick()
  assert.equal(balancesError, false, 'newer background success wins after the foreground error')
  lifecycle.stop()
})

test('风险快照只保留新权威列表中的可请求持仓并复用局部成功', () => {
  const current = {
    live: { value: 1 },
    removed: { value: 2 },
    pending: { value: 3 },
  }
  const next = reconcileMarginRiskSnapshots(current, ['live', 'new-live'], [
    { status: 'rejected', reason: new Error('keep cached live risk') },
    { status: 'fulfilled', value: { value: 4 } },
  ])

  assert.deepEqual(next, {
    live: { value: 1 },
    'new-live': { value: 4 },
  })
  assert.deepEqual(reconcileMarginRiskSnapshots(current, []), {})
})

test('TradeView 用 wallets 权威快照同步钱包与 opened 持仓，再刷新新集合风险', () => {
  const reconciliation = sliceBetween(
    tradeSource,
    'function isMarginPositionRiskEligible',
    'function isCurrentTradingBalancesRequest',
  )
  const foreground = sliceBetween(
    tradeSource,
    'async function loadTradingBalances',
    'function setQuantity',
  )

  assert.match(tradeSource, /createMarginAccountReconciliationLifecycle\(\{[\s\S]*?sessionKey: \(\) => session\.token[\s\S]*?isContractMode: \(\) => mode\.value === 'contract'[\s\S]*?reconcile: reconcileMarginAccount/)
  assert.match(reconciliation, /const margin = await fetchMarginWallets\(\)/)
  assert.match(reconciliation, /position\.status\.trim\(\)\.toLowerCase\(\) === 'opened'/)
  assert.match(reconciliation, /isFilledMarginPosition\(position\)/)
  assert.match(reconciliation, /product\?\.positionRiskSupported !== false/)
  assertOrdered(reconciliation, [
    'marginWallets.value = margin.wallets',
    'marginPositions.value = margin.positions',
    'Promise.allSettled(',
  ])
  assert.match(reconciliation, /reconcileMarginRiskSnapshots\([\s\S]*?eligiblePositionIds/)
  assert.match(reconciliation, /request\.kind === 'background'[\s\S]*?balancesError\.value = false/)

  assert.match(foreground, /balancesLoading\.value = true[\s\S]*?balancesError\.value = false/)
  assert.match(foreground, /marginAccountReconciliation\.refreshForeground\(\)/)
  assert.match(foreground, /if \(result\.state === 'error'\) throw result\.error/)
  assert.match(foreground, /marginWallets\.value = \[\][\s\S]*?marginPositions\.value = \[\][\s\S]*?marginRiskSnapshots\.value = \{\}[\s\S]*?balancesError\.value = true/)

  assert.match(tradeSource, /watch\(\(\) => \[mode\.value, session\.token\] as const,[\s\S]*?marginAccountReconciliation\.invalidate\(\)[\s\S]*?flush: 'sync'/)
  assert.match(tradeSource, /document\.addEventListener\('visibilitychange', handleTradeVisibilityChange\)/)
  assert.match(tradeSource, /document\.removeEventListener\('visibilitychange', handleTradeVisibilityChange\)/)
  assert.match(tradeSource, /marginAccountReconciliation\.invalidate\(\)[\s\S]*?document\.visibilityState === 'hidden'[\s\S]*?marginAccountReconciliation\.refreshBackground\(\{ queueIfBusy: true \}\)/)
  assert.match(tradeSource, /marginAccountReconciliation\.startPolling\(\)/)
  assert.match(tradeSource, /marginAccountReconciliation\.stop\(\)/)
  assert.match(lifecycleSource, /kind === 'background' && !options\.isVisible\(\)/)
  assert.match(lifecycleSource, /kind === 'background' && backgroundInFlight/)
  assert.match(lifecycleSource, /scheduler\.setInterval\([\s\S]*?refreshBackground\(\)/)
  assert.doesNotMatch(tradeSource, /marginRiskRefreshTimer|loadMarginPositionRisks/)

  for (const action of ['performPositionAction', 'performBulkClose', 'submitOrder']) {
    assert.match(sliceFunction(tradeSource, action), /await loadTradingBalances\(\)/)
  }
})

test('TradeView logout guest branch synchronously clears loading before stale login request settles', () => {
  const foreground = sliceBetween(
    tradeSource,
    'async function loadTradingBalances',
    'function setQuantity',
  )
  const guestBranch = sliceBetween(
    foreground,
    'if (!requestSessionKey)',
    'balancesLoading.value = true',
  )

  assertOrdered(foreground, [
    'const requestVersion = ++tradingBalancesRequestVersion',
    'const requestSessionKey = session.token',
    'if (!requestSessionKey)',
  ])
  assertOrdered(guestBranch, [
    'spotWallets.value = []',
    'marginWallets.value = []',
    'marginPositions.value = []',
    'marginRiskSnapshots.value = {}',
    'balancesLoading.value = false',
    'balancesError.value = false',
    'return',
  ])
  assert.match(
    foreground,
    /finally \{\s*if \(isCurrentTradingBalancesRequest\([\s\S]*?balancesLoading\.value = false/,
  )
})

function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T | PromiseLike<T>) => void
  reject: (reason?: unknown) => void
} {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve
    reject = nextReject
  })
  return { promise, resolve, reject }
}

async function tick(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
}

function sliceBetween(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `missing source slice ${start} -> ${end}`)
  return source.slice(startIndex, endIndex)
}

function sliceFunction(source: string, name: string): string {
  const startIndex = source.indexOf(`function ${name}`)
  assert.ok(startIndex >= 0, `missing function ${name}`)
  const nextFunction = source.indexOf('\nfunction ', startIndex + 1)
  const nextAsyncFunction = source.indexOf('\nasync function ', startIndex + 1)
  const candidates = [nextFunction, nextAsyncFunction].filter((index) => index > startIndex)
  const endIndex = candidates.length ? Math.min(...candidates) : source.length
  return source.slice(startIndex, endIndex)
}

function assertOrdered(source: string, fragments: readonly string[]): void {
  let cursor = -1
  fragments.forEach((fragment) => {
    const next = source.indexOf(fragment, cursor + 1)
    assert.ok(next > cursor, `expected ${fragment} after index ${cursor}`)
    cursor = next
  })
}
