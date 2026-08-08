import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createMarketTickerStream,
  type TickerSocket,
  type TickerStreamScheduler,
} from '../src/api/marketTickerStream.ts'

class FakeSocket implements TickerSocket {
  readyState = 0
  readonly sent: string[] = []
  closeCount = 0
  private readonly listeners = new Map<string, Array<(event: { data: unknown }) => void>>()

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    this.closeCount += 1
    this.readyState = 3
  }

  addEventListener(type: string, listener: (event: { data: unknown }) => void): void {
    const current = this.listeners.get(type) ?? []
    current.push(listener)
    this.listeners.set(type, current)
  }

  emit(type: 'open' | 'message' | 'close' | 'error', data: unknown = undefined): void {
    if (type === 'open') this.readyState = 1
    if (type === 'close') this.readyState = 3
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

class FakeScheduler implements TickerStreamScheduler {
  readonly timeouts = new Map<object, () => void>()
  readonly intervals = new Map<object, () => void>()

  setTimeout(callback: () => void): object {
    const handle = {}
    this.timeouts.set(handle, callback)
    return handle
  }

  clearTimeout(handle: unknown): void {
    this.timeouts.delete(handle as object)
  }

  setInterval(callback: () => void): object {
    const handle = {}
    this.intervals.set(handle, callback)
    return handle
  }

  clearInterval(handle: unknown): void {
    this.intervals.delete(handle as object)
  }

  runNextTimeout(): void {
    const entry = this.timeouts.entries().next().value as [object, () => void] | undefined
    assert.ok(entry, 'expected a pending reconnect')
    this.timeouts.delete(entry[0])
    entry[1]()
  }
}

test('ticker stream keeps an exact union of view leases and dispatches only requested symbols', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const firstUpdates: string[] = []
  const secondUpdates: string[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
  })

  const stopFirst = stream.subscribe(['btc/usdt', 'ETH-USDT'], (update) => {
    firstUpdates.push(`${update.symbol}:${update.lastPrice}`)
  })
  const stopSecond = stream.subscribe(['eth_usdt', 'SOLUSDT'], (update) => {
    secondUpdates.push(`${update.symbol}:${update.lastPrice}`)
  })

  assert.equal(sockets.length, 1)
  sockets[0]?.emit('open')
  assert.deepEqual(commands(sockets[0]), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'SOLUSDT' },
  ])

  emitTicker(sockets[0], 'BTC-USDT', 101)
  emitTicker(sockets[0], 'ETHUSDT', 202)
  emitTicker(sockets[0], 'SOL/USDT', 303)
  assert.deepEqual(firstUpdates, ['BTCUSDT:101', 'ETHUSDT:202'])
  assert.deepEqual(secondUpdates, ['ETHUSDT:202', 'SOLUSDT:303'])

  stopFirst()
  stopFirst()
  assert.deepEqual(commands(sockets[0]).at(-1), {
    op: 'unsubscribe',
    channel: 'ticker',
    symbol: 'BTCUSDT',
  })
  emitTicker(sockets[0], 'BTCUSDT', 999)
  emitTicker(sockets[0], 'ETHUSDT', 204)
  assert.deepEqual(firstUpdates, ['BTCUSDT:101', 'ETHUSDT:202'])
  assert.deepEqual(secondUpdates, ['ETHUSDT:202', 'SOLUSDT:303', 'ETHUSDT:204'])

  stopSecond()
  assert.equal(sockets[0]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
})

test('ticker stream rejects late old-connection events and reconnects only current leases', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const updates: number[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 5,
    reconnectMaxMs: 20,
  })

  const stop = stream.subscribe(['BTCUSDT'], (update) => updates.push(update.lastPrice))
  sockets[0]?.emit('open')
  sockets[0]?.emit('error')
  sockets[0]?.emit('close')
  assert.equal(sockets[0]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 1)
  assert.equal(scheduler.intervals.size, 0)

  scheduler.runNextTimeout()
  assert.equal(sockets.length, 2)
  sockets[1]?.emit('open')
  assert.deepEqual(commands(sockets[1]), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
  ])

  emitTicker(sockets[0], 'BTCUSDT', 111)
  emitTicker(sockets[1], 'BTCUSDT', 222)
  assert.deepEqual(updates, [222])

  stop()
  assert.equal(sockets[1]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
})

test('a released lease cannot deliver through its old socket into a new same-symbol lease', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const updates: number[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
  })

  const stopOld = stream.subscribe(['BTCUSDT'], () => assert.fail('old lease must be removed'))
  sockets[0]?.emit('open')
  stopOld()
  const stopNew = stream.subscribe(['BTCUSDT'], (update) => updates.push(update.lastPrice))
  sockets[1]?.emit('open')

  emitTicker(sockets[0], 'BTCUSDT', 111)
  emitTicker(sockets[1], 'BTCUSDT', 222)
  assert.deepEqual(updates, [222])

  stopNew()
})

function emitTicker(socket: FakeSocket | undefined, symbol: string, lastPrice: number): void {
  socket?.emit('message', JSON.stringify({ symbol, last_price: String(lastPrice) }))
}

function commands(socket: FakeSocket | undefined): Array<Record<string, string>> {
  return (socket?.sent ?? [])
    .filter((frame) => frame !== 'ping')
    .map((frame) => JSON.parse(frame) as Record<string, string>)
}
