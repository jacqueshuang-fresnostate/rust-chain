import test from 'node:test'
import assert from 'node:assert/strict'
import { createPinia, setActivePinia } from 'pinia'
import { nextTick } from 'vue'

import { StompService } from '../src/api/stomp.ts'
import { useMarketStore, type Ticker } from '../src/stores/market.ts'

type SentCommand = {
  op: string
  channel: string
  symbol: string
  interval?: string
}

class MockWebSocket {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSED = 3
  static instances: MockWebSocket[] = []

  readyState = MockWebSocket.CONNECTING
  sent: string[] = []
  closeCount = 0
  onopen: (() => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  onerror: ((event: unknown) => void) | null = null
  readonly url: string

  constructor(url: string) {
    this.url = url
    MockWebSocket.instances.push(this)
  }

  open() {
    this.readyState = MockWebSocket.OPEN
    this.onopen?.()
  }

  send(data: string) {
    this.sent.push(data)
  }

  close() {
    this.readyState = MockWebSocket.CLOSED
    this.closeCount++
    this.onclose?.()
  }

  serverClose() {
    this.readyState = MockWebSocket.CLOSED
    this.onclose?.()
  }
}

const originalWebSocket = globalThis.WebSocket
const originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

test('subscribes existing PC market tickers when the public websocket opens', (t) => {
  const { service, store } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  store.setTickers([ticker('BTC/USDT'), ticker('ETH/USDT')])

  service.connect()
  const socket = latestSocket()
  assert.equal(socket.url, 'ws://127.0.0.1:8080/ws/spot')
  assert.deepEqual(sentCommands(socket), [])

  socket.open()

  assert.deepEqual(sentCommands(socket), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' },
  ])

})

test('subscribes market tickers that are loaded after the websocket is connected', async (t) => {
  const { service, store } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  service.connect()
  const socket = latestSocket()
  socket.open()
  assert.deepEqual(sentCommands(socket), [])

  store.setTickers([ticker('SOL/USDT')])
  await nextTick()

  assert.deepEqual(sentCommands(socket), [
    { op: 'subscribe', channel: 'ticker', symbol: 'SOLUSDT' },
  ])

})

test('keeps manual ticker callbacks when an automatic ticker subscription already exists', async (t) => {
  const { service, store } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  store.setTickers([ticker('BTC/USDT')])

  service.connect()
  const socket = latestSocket()
  socket.open()

  const messages: unknown[] = []
  const subscription = await service.subscribe('market', 'market:ticker:BTC/USDT', (message) => {
    messages.push(JSON.parse(message.body))
  })

  const subscribeCommands = sentCommands(socket).filter((command) => command.op === 'subscribe')
  assert.deepEqual(subscribeCommands, [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
  ])

  socket.onmessage?.({
    data: JSON.stringify({
      symbol: 'BTCUSDT',
      last_price: '70001',
      high_24h: '70150',
      low_24h: '69000',
      volume_24h: '12',
      price_change_24h: '1001',
      price_change_percent_24h: '1.45',
      observed_at: 1_717_171_000_000,
    }),
  })

  assert.equal(messages.length, 1)
  assert.equal(store.tickers[0].close, 70001)
  assert.equal(store.tickers[0].high, 70150)
  assert.equal(store.tickers[0].low, 69000)
  assert.equal(store.tickers[0].chg, 1.45)

  subscription.unsubscribe()
})

test('updates existing ticker rows by compact symbol variants', (t) => {
  const { service, store } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  store.setTickers([ticker('BTC/USDT')])

  service.connect()
  const socket = latestSocket()
  socket.open()

  socket.onmessage?.({
    data: JSON.stringify({
      symbol: 'BTC_USDT',
      last_price: '71000',
      high_24h: '72000',
      low_24h: '69000',
      volume_24h: '18',
      price_change_percent_24h: '2.25',
      observed_at: 1_717_172_000_000,
    }),
  })

  assert.equal(store.tickers.length, 1)
  assert.equal(store.tickers[0].symbol, 'BTC/USDT')
  assert.equal(store.tickers[0].close, 71000)
  assert.equal(store.tickers[0].volume, 18)
})

test('routes enveloped depth payloads to matching market subscriptions', async (t) => {
  const { service } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  const messages: any[] = []
  await service.subscribe('market', 'market:depth:BTC/USDT', (message) => {
    messages.push(JSON.parse(message.body))
  })

  const socket = latestSocket()
  socket.open()

  socket.onmessage?.({
    data: JSON.stringify({
      channel: 'depth',
      topic: 'BTCUSDT',
      payload: {
        symbol: 'BTC-USDT',
        bids: [{ price: '70000', amount: '1.5' }],
        asks: [{ price: '70010', quantity: '2' }],
        observed_at: 1_717_173_000_000,
      },
    }),
  })

  assert.equal(messages.length, 1)
  assert.equal(messages[0].symbol, 'BTC/USDT')
  assert.equal(messages[0].direction, 'SNAPSHOT')
  assert.deepEqual(messages[0].bids, [{ price: 70000, amount: 1.5 }])
  assert.deepEqual(messages[0].asks, [{ price: 70010, amount: 2 }])
})

test('routes enveloped kline payloads by topic symbol and interval', async (t) => {
  const { service } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  const messages: any[] = []
  await service.subscribe('market', 'market:kline:BTC/USDT:1min', (message) => {
    messages.push(JSON.parse(message.body))
  })

  const socket = latestSocket()
  socket.open()
  assert.deepEqual(sentCommands(socket), [
    { op: 'subscribe', channel: 'kline', symbol: 'BTCUSDT', interval: '1m' },
  ])

  socket.onmessage?.({
    data: JSON.stringify({
      namespace: 'kline',
      topic: 'BTCUSDT_1m',
      payload: JSON.stringify({
        open_time: 1_717_174_000_000,
        open: '70000',
        high: '71000',
        low: '69900',
        close: '70500',
        volume: '12',
      }),
    }),
  })

  assert.equal(messages.length, 1)
  assert.equal(messages[0].symbol, 'BTCUSDT')
  assert.equal(messages[0].interval, '1m')
  assert.equal(messages[0].time, 1_717_174_000_000)
  assert.equal(messages[0].timestamp, 1_717_174_000_000)
})

test('keeps spot margin and seconds websocket clients isolated', async (t) => {
  const { service } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  const spotMessages: any[] = []
  const marginMessages: any[] = []
  const secondsMessages: any[] = []

  await service.subscribe('spot', 'spot:depth:BTC/USDT', (message) => {
    spotMessages.push(JSON.parse(message.body))
  })
  await service.subscribe('margin', 'margin:depth:BTC/USDT', (message) => {
    marginMessages.push(JSON.parse(message.body))
  })
  await service.subscribe('seconds', 'seconds:ticker:BTC/USDT', (message) => {
    secondsMessages.push(JSON.parse(message.body))
  })

  const [spotSocket, marginSocket, secondsSocket] = MockWebSocket.instances
  assert.ok(spotSocket)
  assert.ok(marginSocket)
  assert.ok(secondsSocket)
  assert.notEqual(spotSocket, marginSocket)
  assert.notEqual(marginSocket, secondsSocket)
  assert.equal(spotSocket.url, 'ws://127.0.0.1:8080/ws/spot')
  assert.equal(marginSocket.url, 'ws://127.0.0.1:8080/ws/margin')
  assert.equal(secondsSocket.url, 'ws://127.0.0.1:8080/ws/seconds')

  spotSocket.open()
  marginSocket.open()
  secondsSocket.open()

  assert.deepEqual(sentCommands(spotSocket), [
    { op: 'subscribe', channel: 'depth', symbol: 'BTCUSDT' },
  ])
  assert.deepEqual(sentCommands(marginSocket), [
    { op: 'subscribe', channel: 'depth', symbol: 'BTCUSDT' },
  ])
  assert.deepEqual(sentCommands(secondsSocket), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
  ])

  service.disconnect('spot')
  assert.equal(spotSocket.closeCount, 1)

  marginSocket.onmessage?.({
    data: JSON.stringify({
      symbol: 'BTCUSDT',
      bids: [{ price: '70000', amount: '1' }],
      asks: [{ price: '70010', amount: '2' }],
    }),
  })
  secondsSocket.onmessage?.({
    data: JSON.stringify({
      symbol: 'BTCUSDT',
      last_price: '70001',
      high_24h: '70100',
      low_24h: '69900',
      volume_24h: '9',
      observed_at: 1_717_175_000_000,
    }),
  })

  assert.equal(spotMessages.length, 0)
  assert.equal(marginMessages.length, 1)
  assert.equal(secondsMessages.length, 1)
})

test('reconnects one business client and resubscribes without touching others', async (t) => {
  const { service } = setupService({ reconnectDelayMs: 1 })
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
  })

  await service.subscribe('margin', 'margin:trade:BTC/USDT', () => undefined)
  await service.subscribe('seconds', 'seconds:ticker:ETH/USDT', () => undefined)

  const [marginSocket, secondsSocket] = MockWebSocket.instances
  marginSocket.open()
  secondsSocket.open()
  assert.deepEqual(sentCommands(marginSocket), [
    { op: 'subscribe', channel: 'trade', symbol: 'BTCUSDT' },
  ])
  assert.deepEqual(sentCommands(secondsSocket), [
    { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' },
  ])

  marginSocket.serverClose()
  await delay(5)

  const reconnectedMarginSocket = latestSocket()
  assert.notEqual(reconnectedMarginSocket, secondsSocket)
  assert.notEqual(reconnectedMarginSocket, marginSocket)
  reconnectedMarginSocket.open()

  assert.deepEqual(sentCommands(reconnectedMarginSocket), [
    { op: 'subscribe', channel: 'trade', symbol: 'BTCUSDT' },
  ])
  assert.equal(secondsSocket.closeCount, 0)
})

test('connects private websocket with stored token and dispatches user events', async (t) => {
  installAuthToken('access token/?=')
  const { service } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
    restoreLocalStorage()
  })

  const messages: string[] = []
  const subscription = await service.subscribePrivate((message) => {
    messages.push(message.body)
  })

  const socket = latestSocket()
  assert.equal(socket.url, 'ws://127.0.0.1:8080/ws/private?token=access%20token%2F%3F%3D')
  socket.open()
  assert.equal(service.isConnected('private'), true)

  socket.onmessage?.({ data: '{"type":"subscribed","channel":"private:user:1"}' })
  assert.deepEqual(messages, [])

  socket.onmessage?.({ data: '{"type":"spot.order.created","order":{"id":1}}' })
  assert.deepEqual(messages, ['{"type":"spot.order.created","order":{"id":1}}'])

  subscription.unsubscribe()
  assert.equal(socket.closeCount, 1)
})

test('does not connect private websocket without a stored token', async (t) => {
  installAuthToken()
  const { service } = setupService()
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
    restoreLocalStorage()
  })

  const subscription = await service.subscribePrivate(() => undefined)

  assert.equal(MockWebSocket.instances.length, 0)
  assert.equal(service.isConnected('private'), false)
  subscription.unsubscribe()
})

test('reconnects private websocket while token remains and stops after token removal', async (t) => {
  installAuthToken('private-token')
  const { service } = setupService({ reconnectDelayMs: 1 })
  t.after(() => {
    service.disconnect()
    restoreWebSocket()
    restoreLocalStorage()
  })

  await service.subscribePrivate(() => undefined)
  const socket = latestSocket()
  socket.open()
  socket.serverClose()
  await delay(5)

  const reconnectedSocket = latestSocket()
  assert.notEqual(reconnectedSocket, socket)
  assert.equal(reconnectedSocket.url, 'ws://127.0.0.1:8080/ws/private?token=private-token')

  localStorage.removeItem('token')
  reconnectedSocket.open()
  reconnectedSocket.serverClose()
  await delay(5)

  assert.equal(MockWebSocket.instances.length, 2)
})

function setupService(options?: { reconnectDelayMs?: number }) {
  MockWebSocket.instances = []
  globalThis.WebSocket = MockWebSocket as unknown as typeof WebSocket
  setActivePinia(createPinia())
  return {
    service: new StompService(options),
    store: useMarketStore(),
  }
}

function restoreWebSocket() {
  globalThis.WebSocket = originalWebSocket
}

function installAuthToken(token?: string) {
  const values = new Map<string, string>()
  if (token) values.set('token', token)

  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value)
      },
      removeItem: (key: string) => {
        values.delete(key)
      },
      clear: () => {
        values.clear()
      },
    },
  })
}

function restoreLocalStorage() {
  if (originalLocalStorageDescriptor) {
    Object.defineProperty(globalThis, 'localStorage', originalLocalStorageDescriptor)
    return
  }
  delete (globalThis as { localStorage?: unknown }).localStorage
}

function latestSocket() {
  const socket = MockWebSocket.instances.at(-1)
  assert.ok(socket)
  return socket
}

function sentCommands(socket: MockWebSocket): SentCommand[] {
  return socket.sent.map((message) => JSON.parse(message) as SentCommand)
}

function delay(ms: number) {
  return new Promise(resolve => setTimeout(resolve, ms))
}

function ticker(symbol: string): Ticker {
  return {
    symbol,
    icon: '',
    open: 70000,
    high: 70000,
    low: 70000,
    close: 70000,
    volume: 10,
    turnover: 700000,
    time: 1_717_170_000_000,
    chg: 0,
    zone: 0,
  }
}
