import { watch, type WatchStopHandle } from 'vue'
import { APP_CONFIG } from '../config/app.ts'
import { mapMarketDepthToTradePlate, mapMarketTickerToPcTicker, mapMarketTradeToPcTrade } from './backendAdapters.ts'
import { useMarketStore, type Ticker } from '../stores/market.ts'

type BusinessEndpoint = 'spot' | 'margin' | 'seconds'
type Endpoint = BusinessEndpoint | 'market' | 'second' | 'swap' | 'private'
type MarketChannel = 'ticker' | 'depth' | 'trade' | 'kline'

type Subscription = {
  unsubscribe: () => void
}

type MessageCallback = (message: { body: string }) => void

type MarketSubscriptionSpec = {
  channel: MarketChannel
  symbol: string
  interval?: string
}

type ActiveSubscription = MarketSubscriptionSpec & {
  callbacks: Map<number, MessageCallback>
  subscribed: boolean
}

type WsClientState = {
  socket: WebSocket | null
  connected: boolean
  manualClose: boolean
  reconnectTimer: ReturnType<typeof setTimeout> | null
  subscriptions: Map<string, ActiveSubscription>
  nextCallbackId: number
  stopTickerWatch: WatchStopHandle | null
  autoTickerSubscriptionIds: Map<string, number>
}

type PrivateWsClientState = {
  socket: WebSocket | null
  connected: boolean
  manualClose: boolean
  reconnectTimer: ReturnType<typeof setTimeout> | null
  callbacks: Map<number, MessageCallback>
  nextCallbackId: number
  token: string | null
}

type IncomingMarketPayload = {
  payload: any
  envelope?: any
}

type StompServiceOptions = {
  reconnectDelayMs?: number
}

export class StompService {
  private clients = new Map<BusinessEndpoint, WsClientState>()
  private privateClient: PrivateWsClientState | null = null
  private reconnectDelayMs: number

  constructor(options: StompServiceOptions = {}) {
    this.reconnectDelayMs = options.reconnectDelayMs ?? 3000
  }

  connect(endpoint: Endpoint = 'spot') {
    if (endpoint === 'private') {
      this.connectPrivate()
      return
    }

    const business = normalizeEndpoint(endpoint)
    if (!business) return

    const client = this.getClient(business)
    if (business === 'spot') this.ensureTickerWatcher(client)
    if (client.connected || client.socket?.readyState === WebSocket.CONNECTING) {
      if (business === 'spot') this.syncTickerSubscriptions(client)
      this.flushSubscriptions(client)
      return
    }

    this.openSocket(business, client)
  }

  connectPrivate() {
    const token = storedAuthToken()
    const client = this.getPrivateClient()
    if (!token) {
      this.disconnectPrivateClient(client, false)
      return
    }

    if (client.token && client.token !== token) {
      this.closePrivateSocket(client, true)
    }

    if (client.connected || client.socket?.readyState === WebSocket.CONNECTING) return

    this.openPrivateSocket(client, token)
  }

  private openSocket(business: BusinessEndpoint, client: WsClientState) {
    this.clearReconnect(client)
    client.manualClose = false
    client.socket = new WebSocket(publicWsUrl(endpointPath(business)))
    client.socket.onopen = () => {
      client.connected = true
      if (business === 'spot') this.syncTickerSubscriptions(client)
      this.flushSubscriptions(client)
    }
    client.socket.onmessage = (event) => this.handleMessage(business, String(event.data))
    client.socket.onclose = () => {
      client.connected = false
      client.socket = null
      client.subscriptions.forEach((subscription) => {
        subscription.subscribed = false
      })
      if (!client.manualClose && client.subscriptions.size > 0) {
        this.scheduleReconnect(business, client)
      }
    }
    client.socket.onerror = (event) => {
      console.error(`[${business}] WebSocket error:`, event)
    }
  }

  private openPrivateSocket(client: PrivateWsClientState, token: string) {
    this.clearPrivateReconnect(client)
    client.manualClose = false
    client.token = token
    client.socket = new WebSocket(privateWsUrl(token))
    client.socket.onopen = () => {
      client.connected = true
    }
    client.socket.onmessage = (event) => this.handlePrivateMessage(client, String(event.data))
    client.socket.onclose = () => {
      client.connected = false
      client.socket = null
      if (!client.manualClose && client.callbacks.size > 0 && storedAuthToken()) {
        this.schedulePrivateReconnect(client)
      }
    }
    client.socket.onerror = (event) => {
      console.error('[private] WebSocket error:', event)
    }
  }

  subscribe(endpoint: Endpoint, topic: string, callback: MessageCallback): Promise<Subscription>
  subscribe(topic: string, callback: MessageCallback): Promise<Subscription>
  subscribe(topicOrEp: string | Endpoint, topicOrCb: string | MessageCallback, cb?: MessageCallback): Promise<Subscription> {
    const endpoint = typeof topicOrCb === 'function' ? 'market' : topicOrEp as Endpoint
    const topic = typeof topicOrCb === 'function' ? topicOrEp : topicOrCb
    const callback = typeof topicOrCb === 'function' ? topicOrCb : cb

    const business = normalizeEndpoint(endpoint)
    if (!callback || !business) {
      return Promise.resolve({ unsubscribe: () => undefined })
    }

    const parsed = parseMarketTopic(String(topic))
    if (!parsed) {
      return Promise.resolve({ unsubscribe: () => undefined })
    }

    const client = this.getClient(business)
    const { key, callbackId } = this.addSubscriptionCallback(client, parsed, callback)
    this.connect(business)
    this.flushSubscriptions(client)

    return Promise.resolve({
      unsubscribe: () => this.removeSubscriptionCallback(client, key, callbackId),
    })
  }

  subscribePrivate(callback: MessageCallback): Promise<Subscription> {
    const token = storedAuthToken()
    if (!token) {
      return Promise.resolve({ unsubscribe: () => undefined })
    }

    const client = this.getPrivateClient()
    const callbackId = client.nextCallbackId++
    client.callbacks.set(callbackId, callback)
    this.connectPrivate()

    return Promise.resolve({
      unsubscribe: () => this.removePrivateCallback(client, callbackId),
    })
  }

  disconnect(endpoint?: Endpoint) {
    if (endpoint) {
      if (endpoint === 'private') {
        this.disconnectPrivate()
        return
      }
      const business = normalizeEndpoint(endpoint)
      if (business) this.disconnectClient(this.getClient(business))
      return
    }

    this.clients.forEach((client) => {
      this.disconnectClient(client)
    })
    this.disconnectPrivate()
  }

  isConnected(endpoint: Endpoint = 'spot'): boolean {
    if (endpoint === 'private') {
      return Boolean(this.privateClient?.connected)
    }
    const business = normalizeEndpoint(endpoint)
    return Boolean(business && this.getClient(business).connected)
  }

  disconnectPrivate() {
    this.disconnectPrivateClient(this.getPrivateClient(), true)
  }

  private disconnectClient(client: WsClientState) {
    client.manualClose = true
    this.clearReconnect(client)
    client.subscriptions.forEach((subscription) => {
      if (subscription.subscribed) this.sendCommand(client, 'unsubscribe', subscription)
    })
    client.subscriptions.clear()
    client.autoTickerSubscriptionIds.clear()
    client.stopTickerWatch?.()
    client.stopTickerWatch = null
    const socket = client.socket
    client.socket = null
    client.connected = false
    socket?.close()
  }

  private disconnectPrivateClient(client: PrivateWsClientState, clearCallbacks: boolean) {
    if (clearCallbacks) {
      client.callbacks.clear()
    }
    this.closePrivateSocket(client, true)
  }

  private closePrivateSocket(client: PrivateWsClientState, manualClose: boolean) {
    client.manualClose = manualClose
    this.clearPrivateReconnect(client)
    const socket = client.socket
    client.socket = null
    client.connected = false
    client.token = null
    socket?.close()
  }

  private flushSubscriptions(client: WsClientState) {
    if (!client.connected || !client.socket || client.socket.readyState !== WebSocket.OPEN) return

    client.subscriptions.forEach((subscription) => {
      if (!subscription.subscribed && this.sendCommand(client, 'subscribe', subscription)) {
        subscription.subscribed = true
      }
    })
  }

  private addSubscriptionCallback(client: WsClientState, spec: MarketSubscriptionSpec, callback: MessageCallback) {
    const key = subscriptionKey(spec.channel, spec.symbol, spec.interval)
    let subscription = client.subscriptions.get(key)
    if (!subscription) {
      subscription = {
        ...spec,
        symbol: compactMarketSymbol(spec.symbol),
        callbacks: new Map(),
        subscribed: false,
      }
      client.subscriptions.set(key, subscription)
    }

    const callbackId = client.nextCallbackId++
    subscription.callbacks.set(callbackId, callback)
    return { key, callbackId }
  }

  private removeSubscriptionCallback(client: WsClientState, key: string, callbackId: number) {
    const subscription = client.subscriptions.get(key)
    if (!subscription) return

    subscription.callbacks.delete(callbackId)
    if (subscription.callbacks.size > 0) return

    if (subscription.subscribed) this.sendCommand(client, 'unsubscribe', subscription)
    client.subscriptions.delete(key)
  }

  private ensureTickerWatcher(client: WsClientState) {
    if (client.stopTickerWatch) return

    const store = useMarketStore()
    client.stopTickerWatch = watch(
      () => store.tickers.map((ticker) => compactMarketSymbol(ticker.symbol)).filter(Boolean).sort().join('|'),
      () => this.syncTickerSubscriptions(client),
      { immediate: true },
    )
  }

  private syncTickerSubscriptions(client: WsClientState) {
    const store = useMarketStore()
    const symbols = new Set(
      store.tickers
        .map((ticker) => compactMarketSymbol(ticker.symbol))
        .filter(Boolean),
    )

    client.autoTickerSubscriptionIds.forEach((callbackId, symbol) => {
      if (symbols.has(symbol)) return
      this.removeSubscriptionCallback(client, subscriptionKey('ticker', symbol), callbackId)
      client.autoTickerSubscriptionIds.delete(symbol)
    })

    symbols.forEach((symbol) => {
      if (client.autoTickerSubscriptionIds.has(symbol)) return
      const { callbackId } = this.addSubscriptionCallback(client, { channel: 'ticker', symbol }, () => undefined)
      client.autoTickerSubscriptionIds.set(symbol, callbackId)
    })

    this.flushSubscriptions(client)
  }

  private handleMessage(business: BusinessEndpoint, body: string) {
    if (body === 'pong') return

    let payload: any
    try {
      payload = JSON.parse(body)
    } catch {
      return
    }

    if (payload.type === 'subscribed' || payload.type === 'unsubscribed' || payload.type === 'error') return

    unwrapMarketPayloads(payload).forEach(({ payload: messagePayload, envelope }) => {
      const channel = detectChannel(messagePayload, envelope)
      if (!channel) return

      const symbol = resolveMarketSymbol(messagePayload, envelope, channel)
      if (!symbol) return

      const interval = channel === 'kline' ? resolveKlineInterval(messagePayload, envelope) : undefined
      const client = this.getClient(business)
      const subscription = client.subscriptions.get(subscriptionKey(channel, symbol, interval))
      if (!subscription) return

      const adapted = adaptPayload(business, channel, enrichMarketPayload(messagePayload, symbol, interval))
      subscription.callbacks.forEach((callback) => {
        callback({ body: JSON.stringify(adapted) })
      })
    })
  }

  private handlePrivateMessage(client: PrivateWsClientState, body: string) {
    if (body === 'pong') return

    let payload: any
    try {
      payload = JSON.parse(body)
    } catch {
      payload = null
    }

    if (payload?.type === 'subscribed' || payload?.type === 'unsubscribed' || payload?.type === 'error') return

    client.callbacks.forEach((callback) => {
      callback({ body })
    })
  }

  private sendCommand(client: WsClientState, op: 'subscribe' | 'unsubscribe', subscription: MarketSubscriptionSpec): boolean {
    if (!client.socket || client.socket.readyState !== WebSocket.OPEN) return false
    client.socket.send(JSON.stringify({
      op,
      channel: subscription.channel,
      symbol: subscription.symbol,
      ...(subscription.interval ? { interval: subscription.interval } : {}),
    }))
    return true
  }

  private getClient(endpoint: BusinessEndpoint): WsClientState {
    let client = this.clients.get(endpoint)
    if (!client) {
      client = {
        socket: null,
        connected: false,
        manualClose: false,
        reconnectTimer: null,
        subscriptions: new Map(),
        nextCallbackId: 1,
        stopTickerWatch: null,
        autoTickerSubscriptionIds: new Map(),
      }
      this.clients.set(endpoint, client)
    }
    return client
  }

  private getPrivateClient(): PrivateWsClientState {
    if (!this.privateClient) {
      this.privateClient = {
        socket: null,
        connected: false,
        manualClose: false,
        reconnectTimer: null,
        callbacks: new Map(),
        nextCallbackId: 1,
        token: null,
      }
    }
    return this.privateClient
  }

  private removePrivateCallback(client: PrivateWsClientState, callbackId: number) {
    client.callbacks.delete(callbackId)
    if (client.callbacks.size === 0) {
      this.disconnectPrivateClient(client, false)
    }
  }

  private scheduleReconnect(business: BusinessEndpoint, client: WsClientState) {
    if (client.reconnectTimer) return
    client.reconnectTimer = setTimeout(() => {
      client.reconnectTimer = null
      if (client.manualClose || client.subscriptions.size === 0) return
      this.openSocket(business, client)
    }, this.reconnectDelayMs)
  }

  private schedulePrivateReconnect(client: PrivateWsClientState) {
    if (client.reconnectTimer) return
    client.reconnectTimer = setTimeout(() => {
      client.reconnectTimer = null
      if (client.manualClose || client.callbacks.size === 0 || !storedAuthToken()) return
      this.connectPrivate()
    }, this.reconnectDelayMs)
  }

  private clearReconnect(client: WsClientState) {
    if (!client.reconnectTimer) return
    clearTimeout(client.reconnectTimer)
    client.reconnectTimer = null
  }

  private clearPrivateReconnect(client: PrivateWsClientState) {
    if (!client.reconnectTimer) return
    clearTimeout(client.reconnectTimer)
    client.reconnectTimer = null
  }
}

function parseMarketTopic(topic: string): MarketSubscriptionSpec | null {
  const [module, channel, rawSymbol, interval] = topic.split(':')
  if (!isSupportedTopicModule(module) || !rawSymbol) return null

  if (channel === 'ticker') {
    return { channel: 'ticker', symbol: compactMarketSymbol(rawSymbol) }
  }
  if (channel === 'kline') {
    return { channel: 'kline', symbol: compactMarketSymbol(rawSymbol), interval: normalizeKlineInterval(interval || '1m') }
  }
  if (channel === 'trade') {
    return { channel: 'trade', symbol: compactMarketSymbol(rawSymbol) }
  }
  if (channel === 'depth') {
    return { channel: 'depth', symbol: compactMarketSymbol(rawSymbol) }
  }

  return null
}

function unwrapMarketPayloads(payload: any): IncomingMarketPayload[] {
  const parsedPayload = parseJsonPayload(payload)
  if (Array.isArray(parsedPayload)) {
    return parsedPayload.map((item) => ({ payload: parseJsonPayload(item) }))
  }
  if (!isRecord(parsedPayload)) return []

  const nested = findNestedMarketPayload(parsedPayload)
  if (nested === undefined) return [{ payload: parsedPayload, envelope: parsedPayload }]

  const parsedNested = parseJsonPayload(nested)
  if (Array.isArray(parsedNested)) {
    return parsedNested.map((item) => ({ payload: parseJsonPayload(item), envelope: parsedPayload }))
  }
  return [{ payload: parsedNested, envelope: parsedPayload }]
}

function findNestedMarketPayload(payload: Record<string, any>) {
  const envelopeChannel = firstMarketChannel(payload.channel, payload.namespace, payload.topic, payload.destination)
  for (const key of ['payload', 'data', 'body', 'message']) {
    if (!(key in payload) || payload[key] === null || payload[key] === undefined) continue
    const nested = parseJsonPayload(payload[key])
    if (looksLikeMarketPayload(nested) || envelopeChannel || !detectChannel(payload)) return nested
  }
  return undefined
}

function parseJsonPayload(payload: any): any {
  if (typeof payload !== 'string') return payload
  try {
    return JSON.parse(payload)
  } catch {
    return payload
  }
}

function detectChannel(payload: any, envelope?: any): MarketChannel | null {
  const explicit = firstMarketChannel(
    payload?.channel,
    payload?.namespace,
    payload?.type,
    envelope?.channel,
    envelope?.namespace,
    envelope?.type,
    envelope?.topic,
  )
  if (explicit) return explicit

  const sample = Array.isArray(payload) ? payload[0] : payload
  if (!isRecord(sample)) return null

  if ('last_price' in sample && 'volume_24h' in sample) return 'ticker'
  if ('bids' in sample && 'asks' in sample) return 'depth'
  if ('open_time' in sample && ('interval' in sample || explicitMarketChannel(envelope?.topic) === 'kline')) return 'kline'
  if ('trade_id' in sample || 'traded_at' in sample || 'side' in sample || 'direction' in sample) return 'trade'
  return null
}

function adaptPayload(business: BusinessEndpoint, channel: MarketChannel, payload: any) {
  if (channel === 'ticker') {
    const normalizedPayload = {
      ...payload,
      symbol: compactMarketSymbol(payload.symbol || ''),
    }
    const ticker = mapMarketTickerToPcTicker(findCurrentTicker(normalizedPayload.symbol), normalizedPayload)
    if (business === 'spot') updateMarketTicker(ticker)
    return ticker
  }
  if (channel === 'depth') {
    const depth = mapMarketDepthToTradePlate(payload)
    return {
      ...depth,
      direction: 'SNAPSHOT',
      items: depth.bids,
    }
  }
  if (channel === 'trade') {
    const trades = Array.isArray(payload) ? payload : [payload]
    const mappedTrades = trades.map((trade) => mapMarketTradeToPcTrade({
      ...trade,
      id: trade.id ?? trade.trade_id,
    }))
    return Array.isArray(payload) ? mappedTrades : mappedTrades[0]
  }
  return {
    ...payload,
    time: payload.open_time,
    timestamp: payload.open_time,
  }
}

function updateMarketTicker(ticker: Ticker) {
  const store = useMarketStore()
  store.updateTicker(ticker)
}

function findCurrentTicker(symbol: string): Ticker | undefined {
  const store = useMarketStore()
  const compactSymbol = compactMarketSymbol(symbol)
  return store.tickers.find((ticker) => compactMarketSymbol(ticker.symbol) === compactSymbol)
}

function resolveMarketSymbol(payload: any, envelope: any, channel: MarketChannel): string {
  const sample = Array.isArray(payload) ? payload[0] : payload
  const directSymbol = firstString(
    sample?.symbol,
    sample?.pair_id,
    sample?.pair,
    sample?.market,
    sample?.instId,
    envelope?.symbol,
    envelope?.pair_id,
    envelope?.pair,
    envelope?.market,
    envelope?.instId,
  )
  if (directSymbol) return compactMarketSymbol(directSymbol)

  const topicSymbol = symbolFromTopic(
    firstString(envelope?.topic, envelope?.channel, envelope?.destination, sample?.topic, sample?.channel),
    channel,
  )
  return topicSymbol ? compactMarketSymbol(topicSymbol) : ''
}

function resolveKlineInterval(payload: any, envelope: any): string {
  const sample = Array.isArray(payload) ? payload[0] : payload
  const directInterval = firstString(sample?.interval, sample?.resolution, envelope?.interval)
  if (directInterval) return normalizeKlineInterval(directInterval)

  const topic = firstString(envelope?.topic, envelope?.channel, sample?.topic, sample?.channel)
  const interval = intervalFromTopic(topic)
  return normalizeKlineInterval(interval || '1m')
}

function enrichMarketPayload(payload: any, symbol: string, interval?: string): any {
  if (Array.isArray(payload)) {
    return payload.map((item) => enrichMarketPayload(item, symbol, interval))
  }
  if (!isRecord(payload)) return payload
  return {
    ...payload,
    symbol: compactMarketSymbol(firstString(payload.symbol, payload.pair_id, payload.pair, payload.market, payload.instId) || symbol),
    ...(interval ? { interval: firstString(payload.interval, payload.resolution) || interval } : {}),
  }
}

function publicWsUrl(path: string): string {
  const domain = APP_CONFIG.BACKEND_API_DOMAIN.replace(/\/$/, '')
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  return `${domain}${normalizedPath}`.replace(/^http/, 'ws')
}

function privateWsUrl(token: string): string {
  return publicWsUrl(`/ws/private?token=${encodeURIComponent(token)}`)
}

function storedAuthToken(): string {
  try {
    return globalThis.localStorage?.getItem('token')?.trim() || ''
  } catch {
    return ''
  }
}

function normalizeEndpoint(endpoint: Endpoint): BusinessEndpoint | null {
  if (endpoint === 'spot' || endpoint === 'market') return 'spot'
  if (endpoint === 'margin' || endpoint === 'swap') return 'margin'
  if (endpoint === 'seconds' || endpoint === 'second') return 'seconds'
  return null
}

function endpointPath(endpoint: BusinessEndpoint): string {
  return `/ws/${endpoint}`
}

function isSupportedTopicModule(module: string): boolean {
  return module === 'market'
    || module === 'spot'
    || module === 'margin'
    || module === 'seconds'
    || module === 'second'
    || module === 'swap'
}

function normalizeKlineInterval(resolution: string): string {
  const normalized = resolution.trim().toLowerCase()
  if (normalized.endsWith('min')) return `${Number.parseInt(normalized, 10) || 1}m`
  if (normalized === '1day') return '1d'
  return normalized
}

function compactMarketSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function subscriptionKey(channel: MarketChannel, symbol: string, interval?: string): string {
  return `${channel}:${compactMarketSymbol(symbol)}:${interval || ''}`
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function firstString(...values: unknown[]): string {
  for (const value of values) {
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return ''
}

function firstMarketChannel(...values: unknown[]): MarketChannel | null {
  for (const value of values) {
    const channel = explicitMarketChannel(value)
    if (channel) return channel
  }
  return null
}

function explicitMarketChannel(value: unknown): MarketChannel | null {
  if (typeof value !== 'string') return null
  const normalized = value.toLowerCase()
  if (normalized.includes('ticker')) return 'ticker'
  if (normalized.includes('depth')) return 'depth'
  if (normalized.includes('trade')) return 'trade'
  if (normalized.includes('kline')) return 'kline'
  return null
}

function looksLikeMarketPayload(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(looksLikeMarketPayload)
  return Boolean(detectChannel(value))
}

function symbolFromTopic(topic: string, channel: MarketChannel): string {
  if (!topic) return ''
  const parts = topic.split(/[:/]/).map((part) => part.trim()).filter(Boolean)
  const channelIndex = parts.findIndex((part) => explicitMarketChannel(part) === channel)
  if (channelIndex >= 0) return parts[channelIndex + 1] ? stripIntervalFromTopic(parts[channelIndex + 1]) : ''
  const candidate = stripIntervalFromTopic(parts.at(-1) || topic)
  return explicitMarketChannel(candidate) ? '' : candidate
}

function intervalFromTopic(topic: string): string {
  if (!topic) return ''
  const match = topic.match(/[_:](\d+min|\d+[mhdw]|1day)$/i)
  return match?.[1] || ''
}

function stripIntervalFromTopic(value: string): string {
  return value.replace(/[_:](\d+min|\d+[mhdw]|1day)$/i, '')
}

export const stompService = new StompService()
