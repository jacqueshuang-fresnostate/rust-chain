import { APP_CONFIG } from '@/config/app.ts'
import { mapMarketDepthToTradePlate, mapMarketTickerToPcTicker, mapMarketTradeToPcTrade } from './backendAdapters'
import { useMarketStore, type Ticker } from '@/stores/market'

type Endpoint = 'market' | 'second' | 'swap'
type MarketChannel = 'ticker' | 'depth' | 'trade' | 'kline'

type Subscription = {
  unsubscribe: () => void
}

type MessageCallback = (message: { body: string }) => void

type PendingSubscription = {
  channel: MarketChannel
  symbol: string
  interval?: string
  callback: MessageCallback
  resolve: (sub: Subscription) => void
}

class StompService {
  private socket: WebSocket | null = null
  private connected = false
  private pendingSubscriptions: PendingSubscription[] = []
  private subscriptions = new Map<string, PendingSubscription>()

  connect(endpoint: Endpoint = 'market') {
    if (endpoint !== 'market') return
    if (this.connected || this.socket?.readyState === WebSocket.CONNECTING) return

    this.socket = new WebSocket(publicWsUrl('/ws/public'))
    this.socket.onopen = () => {
      this.connected = true
      this.flushPendingSubscriptions()
    }
    this.socket.onmessage = (event) => this.handleMessage(String(event.data))
    this.socket.onclose = () => {
      this.connected = false
      this.socket = null
    }
    this.socket.onerror = (event) => {
      console.error('[market] WebSocket error:', event)
    }
  }

  subscribe(endpoint: Endpoint, topic: string, callback: MessageCallback): Promise<Subscription>
  subscribe(topic: string, callback: MessageCallback): Promise<Subscription>
  subscribe(topicOrEp: string | Endpoint, topicOrCb: string | MessageCallback, cb?: MessageCallback): Promise<Subscription> {
    const endpoint = typeof topicOrCb === 'function' ? 'market' : topicOrEp as Endpoint
    const topic = typeof topicOrCb === 'function' ? topicOrEp : topicOrCb
    const callback = typeof topicOrCb === 'function' ? topicOrCb : cb

    if (!callback || endpoint !== 'market') {
      return Promise.resolve({ unsubscribe: () => undefined })
    }

    const parsed = parseMarketTopic(String(topic), callback)
    if (!parsed) {
      return Promise.resolve({ unsubscribe: () => undefined })
    }

    return new Promise((resolve) => {
      const pending = { ...parsed, resolve }
      this.pendingSubscriptions.push(pending)
      this.connect('market')
      this.flushPendingSubscriptions()
    })
  }

  disconnect(endpoint?: Endpoint) {
    if (endpoint && endpoint !== 'market') return
    this.subscriptions.forEach((subscription) => this.sendCommand('unsubscribe', subscription))
    this.subscriptions.clear()
    this.pendingSubscriptions = []
    this.socket?.close()
    this.socket = null
    this.connected = false
  }

  isConnected(endpoint: Endpoint = 'market'): boolean {
    return endpoint === 'market' && this.connected
  }

  private flushPendingSubscriptions() {
    if (!this.connected || !this.socket || this.socket.readyState !== WebSocket.OPEN) return

    while (this.pendingSubscriptions.length > 0) {
      const subscription = this.pendingSubscriptions.shift()
      if (!subscription) continue
      const key = subscriptionKey(subscription.channel, subscription.symbol, subscription.interval)
      this.subscriptions.set(key, subscription)
      this.sendCommand('subscribe', subscription)
      subscription.resolve({
        unsubscribe: () => {
          this.subscriptions.delete(key)
          this.sendCommand('unsubscribe', subscription)
        },
      })
    }
  }

  private handleMessage(body: string) {
    if (body === 'pong') return

    let payload: any
    try {
      payload = JSON.parse(body)
    } catch {
      return
    }

    if (payload.type === 'subscribed' || payload.type === 'unsubscribed' || payload.type === 'error') return

    const channel = detectChannel(payload)
    if (!channel) return
    const symbol = compactMarketSymbol(payload.symbol || '')
    const interval = channel === 'kline' ? String(payload.interval || '1m') : undefined
    const subscription = this.subscriptions.get(subscriptionKey(channel, symbol, interval))
    if (!subscription) return

    const adapted = adaptPayload(channel, payload)
    subscription.callback({ body: JSON.stringify(adapted) })
  }

  private sendCommand(op: 'subscribe' | 'unsubscribe', subscription: Pick<PendingSubscription, 'channel' | 'symbol' | 'interval'>) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return
    this.socket.send(JSON.stringify({
      op,
      channel: subscription.channel,
      symbol: subscription.symbol,
      ...(subscription.interval ? { interval: subscription.interval } : {}),
    }))
  }
}

function parseMarketTopic(topic: string, callback: MessageCallback): Omit<PendingSubscription, 'resolve'> | null {
  const [module, channel, rawSymbol, interval] = topic.split(':')
  if (module !== 'market' || !rawSymbol) return null

  if (channel === 'ticker') {
    return { channel: 'ticker', symbol: compactMarketSymbol(rawSymbol), callback }
  }
  if (channel === 'kline') {
    return { channel: 'kline', symbol: compactMarketSymbol(rawSymbol), interval: normalizeKlineInterval(interval || '1m'), callback }
  }
  if (channel === 'trade') {
    return { channel: 'trade', symbol: compactMarketSymbol(rawSymbol), callback }
  }
  if (channel === 'depth') {
    return { channel: 'depth', symbol: compactMarketSymbol(rawSymbol), callback }
  }

  return null
}

function detectChannel(payload: any): MarketChannel | null {
  if ('last_price' in payload && 'volume_24h' in payload) return 'ticker'
  if ('bids' in payload && 'asks' in payload) return 'depth'
  if ('open_time' in payload && 'interval' in payload) return 'kline'
  if ('trade_id' in payload || 'traded_at' in payload || 'side' in payload) return 'trade'
  return null
}

function adaptPayload(channel: MarketChannel, payload: any) {
  if (channel === 'ticker') {
    const ticker = mapMarketTickerToPcTicker(undefined, payload)
    updateMarketTicker(ticker)
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
    return mapMarketTradeToPcTrade({
      ...payload,
      id: payload.id ?? payload.trade_id,
    })
  }
  return {
    ...payload,
    time: payload.open_time,
    timestamp: payload.open_time,
  }
}

function updateMarketTicker(ticker: Ticker) {
  const store = useMarketStore()
  const current = store.tickers.find((item) => item.symbol === ticker.symbol)
  store.updateTicker(mapMarketTickerToPcTicker(current, {
    symbol: compactMarketSymbol(ticker.symbol),
    last_price: ticker.close,
    volume_24h: ticker.volume,
    observed_at: ticker.time,
  }))
}

function publicWsUrl(path: string): string {
  const domain = APP_CONFIG.BACKEND_API_DOMAIN.replace(/\/$/, '')
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  return `${domain}${normalizedPath}`.replace(/^http/, 'ws')
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

export const stompService = new StompService()
