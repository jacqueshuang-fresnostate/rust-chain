import { buildWebSocketUrl } from '../config/backend';
import { canonicalDecimalText } from '../shared/decimal';

export type MarketTickerStatus = 'connecting' | 'fresh' | 'offline' | 'stale';

export type MarketTickerSnapshot = {
  lastPrice: string | null;
  observedAt?: number;
  receivedAt?: number;
  status: MarketTickerStatus;
  symbol: string;
};

type TickerPayload = {
  last_price?: unknown;
  observed_at?: unknown;
  symbol?: unknown;
};

type TickerListener = (payload: MarketTickerSnapshot) => void;

type SymbolEntry = {
  lastPrice: string | null;
  listeners: Set<TickerListener>;
  observedAt?: number;
  receivedAt?: number;
  status: MarketTickerStatus;
  symbol: string;
};

export type MarketTickerManagerOptions = {
  connectTimeoutMs?: number;
  freshnessMs?: number;
  heartbeatMs?: number;
  idleDisconnectMs?: number;
  inboundTimeoutMs?: number;
  now?: () => number;
  random?: () => number;
  reconnectBaseMs?: number;
  reconnectMaxMs?: number;
  webSocketFactory?: (url: string) => WebSocket;
};

const DEFAULT_FRESHNESS_MS = 30_000;
const DEFAULT_CONNECT_TIMEOUT_MS = 15_000;
const DEFAULT_HEARTBEAT_MS = 20_000;
const DEFAULT_IDLE_DISCONNECT_MS = 500;
const DEFAULT_INBOUND_TIMEOUT_MS = 55_000;

export function normalizeTickerSymbol(symbol: string) {
  return symbol
    .trim()
    .split('')
    .filter((character) => /[A-Za-z0-9]/.test(character))
    .join('')
    .toUpperCase();
}

function parseTickerMessage(data: unknown) {
  if (typeof data !== 'string' || data === 'pong') return null;
  try {
    const payload = JSON.parse(data) as TickerPayload;
    if (typeof payload.symbol !== 'string' || typeof payload.last_price !== 'string') return null;
    const symbol = normalizeTickerSymbol(payload.symbol);
    const lastPrice = canonicalDecimalText(payload.last_price);
    if (!symbol || lastPrice === null) return null;
    return {
      symbol,
      lastPrice,
      observedAt:
        typeof payload.observed_at === 'number' && Number.isSafeInteger(payload.observed_at) && payload.observed_at >= 0
          ? payload.observed_at
          : undefined
    };
  } catch {
    return null;
  }
}

function snapshot(entry: SymbolEntry): MarketTickerSnapshot {
  return {
    lastPrice: entry.lastPrice,
    observedAt: entry.observedAt,
    receivedAt: entry.receivedAt,
    status: entry.status,
    symbol: entry.symbol
  };
}

export class MarketTickerConnectionManager {
  private readonly connectTimeoutMs: number;
  private connectTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly entries = new Map<string, SymbolEntry>();
  private readonly freshnessMs: number;
  private generation = 0;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private readonly heartbeatMs: number;
  private idleDisconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly idleDisconnectMs: number;
  private inboundAt: number | null = null;
  private readonly inboundTimeoutMs: number;
  private listenersAttached = false;
  private readonly now: () => number;
  private readonly random: () => number;
  private reconnectAttempt = 0;
  private readonly reconnectBaseMs: number;
  private readonly reconnectMaxMs: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private socket: WebSocket | null = null;
  private readonly webSocketFactory: (url: string) => WebSocket;
  private watchdogTimer: ReturnType<typeof setInterval> | null = null;

  constructor(options: MarketTickerManagerOptions = {}) {
    this.connectTimeoutMs = options.connectTimeoutMs ?? DEFAULT_CONNECT_TIMEOUT_MS;
    this.freshnessMs = options.freshnessMs ?? DEFAULT_FRESHNESS_MS;
    this.heartbeatMs = options.heartbeatMs ?? DEFAULT_HEARTBEAT_MS;
    this.idleDisconnectMs = options.idleDisconnectMs ?? DEFAULT_IDLE_DISCONNECT_MS;
    this.inboundTimeoutMs = options.inboundTimeoutMs ?? DEFAULT_INBOUND_TIMEOUT_MS;
    this.now = options.now ?? Date.now;
    this.random = options.random ?? Math.random;
    this.reconnectBaseMs = options.reconnectBaseMs ?? 1_000;
    this.reconnectMaxMs = options.reconnectMaxMs ?? 30_000;
    this.webSocketFactory = options.webSocketFactory ?? ((url) => new window.WebSocket(url));
  }

  subscribe(symbol: string, listener: TickerListener): () => void {
    const normalizedSymbol = normalizeTickerSymbol(symbol);
    if (!normalizedSymbol || typeof window === 'undefined') return () => {};
    if (this.idleDisconnectTimer) globalThis.clearTimeout(this.idleDisconnectTimer);
    this.idleDisconnectTimer = null;

    let entry = this.entries.get(normalizedSymbol);
    if (!entry) {
      entry = { lastPrice: null, listeners: new Set(), status: 'connecting', symbol: normalizedSymbol };
      this.entries.set(normalizedSymbol, entry);
      this.sendSubscription('subscribe', normalizedSymbol);
    }
    entry.listeners.add(listener);
    listener(snapshot(entry));
    this.attachEnvironmentListeners();
    this.ensureConnected();

    let active = true;
    return () => {
      if (!active) return;
      active = false;
      const current = this.entries.get(normalizedSymbol);
      if (!current) return;
      current.listeners.delete(listener);
      if (current.listeners.size === 0) {
        this.sendSubscription('unsubscribe', normalizedSymbol);
        this.entries.delete(normalizedSymbol);
      }
      if (this.entries.size === 0) this.scheduleIdleStop();
    };
  }

  reset(): void {
    this.entries.clear();
    this.stop();
  }

  private notify(entry: SymbolEntry): void {
    const value = snapshot(entry);
    entry.listeners.forEach((listener) => listener(value));
  }

  private setAllStatus(status: MarketTickerStatus): void {
    this.entries.forEach((entry) => {
      if (entry.status === status) return;
      entry.status = status;
      this.notify(entry);
    });
  }

  private isOpen(): boolean {
    return Boolean(this.socket && this.socket.readyState === 1);
  }

  private sendSubscription(op: 'subscribe' | 'unsubscribe', symbol: string): void {
    if (!this.isOpen()) return;
    this.socket?.send(JSON.stringify({ op, channel: 'ticker', symbol }));
  }

  private ensureConnected(): void {
    if (this.entries.size === 0 || this.socket || this.reconnectTimer) return;
    if (typeof navigator !== 'undefined' && navigator.onLine === false) {
      this.setAllStatus('offline');
      return;
    }
    this.connect();
  }

  private connect(): void {
    if (this.entries.size === 0 || this.socket) return;
    const generation = ++this.generation;
    this.setAllStatus('connecting');
    let socket: WebSocket;
    try {
      socket = this.webSocketFactory(buildWebSocketUrl('/ws/public'));
    } catch {
      this.scheduleReconnect(generation);
      return;
    }
    this.socket = socket;
    this.connectTimer = globalThis.setTimeout(() => {
      if (generation !== this.generation || socket !== this.socket || socket.readyState === 1) return;
      this.disconnectForReconnect(generation, socket);
    }, this.connectTimeoutMs);

    socket.addEventListener('open', () => {
      if (generation !== this.generation || socket !== this.socket) return;
      this.clearConnectTimer();
      this.reconnectAttempt = 0;
      this.inboundAt = this.now();
      this.entries.forEach((entry) => this.sendSubscription('subscribe', entry.symbol));
      this.startLiveness(generation, socket);
    });
    socket.addEventListener('message', (event) => {
      if (generation !== this.generation || socket !== this.socket) return;
      this.inboundAt = this.now();
      const payload = parseTickerMessage((event as MessageEvent<unknown>).data);
      if (!payload) return;
      const entry = this.entries.get(payload.symbol);
      if (!entry) return;
      entry.lastPrice = payload.lastPrice;
      entry.observedAt = payload.observedAt;
      entry.receivedAt = this.now();
      entry.status = 'fresh';
      this.notify(entry);
    });
    socket.addEventListener('error', () => {
      this.disconnectForReconnect(generation, socket);
    });
    socket.addEventListener('close', () => {
      if (generation !== this.generation || socket !== this.socket) return;
      this.clearConnectTimer();
      this.socket = null;
      this.clearLiveness();
      if (this.entries.size === 0) return;
      this.setAllStatus('offline');
      this.scheduleReconnect(generation);
    });
  }

  private disconnectForReconnect(generation: number, socket: WebSocket): void {
    if (generation !== this.generation || socket !== this.socket) return;
    this.clearConnectTimer();
    this.socket = null;
    this.clearLiveness();
    try {
      if (socket.readyState !== 3) socket.close();
    } catch {
      // 关闭失败不得阻断新 generation 的重连。
    }
    if (this.entries.size === 0) return;
    this.setAllStatus('offline');
    this.scheduleReconnect(generation);
  }

  private startLiveness(generation: number, socket: WebSocket): void {
    this.clearLiveness();
    this.heartbeatTimer = globalThis.setInterval(() => {
      if (generation !== this.generation || socket !== this.socket || !this.isOpen()) return;
      socket.send('ping');
    }, this.heartbeatMs);
    const watchdogInterval = Math.max(500, Math.min(5_000, Math.floor(this.freshnessMs / 2)));
    this.watchdogTimer = globalThis.setInterval(() => {
      if (generation !== this.generation || socket !== this.socket) return;
      const now = this.now();
      this.entries.forEach((entry) => {
        if (entry.status === 'fresh' && entry.receivedAt !== undefined && now - entry.receivedAt > this.freshnessMs) {
          entry.status = 'stale';
          this.notify(entry);
        }
      });
      if (this.inboundAt !== null && now - this.inboundAt > this.inboundTimeoutMs) {
        this.disconnectForReconnect(generation, socket);
      }
    }, watchdogInterval);
  }

  private scheduleReconnect(generation: number): void {
    if (generation !== this.generation || this.entries.size === 0 || this.reconnectTimer) return;
    const exponential = Math.min(this.reconnectMaxMs, this.reconnectBaseMs * 2 ** Math.min(this.reconnectAttempt, 10));
    const delay = Math.round(exponential * (0.8 + this.random() * 0.4));
    this.reconnectAttempt += 1;
    this.reconnectTimer = globalThis.setTimeout(() => {
      this.reconnectTimer = null;
      if (generation !== this.generation || this.entries.size === 0) return;
      this.ensureConnected();
    }, delay);
  }

  private scheduleIdleStop(): void {
    if (this.idleDisconnectTimer) return;
    this.idleDisconnectTimer = globalThis.setTimeout(() => {
      this.idleDisconnectTimer = null;
      if (this.entries.size === 0) this.stop();
    }, this.idleDisconnectMs);
  }

  private clearLiveness(): void {
    if (this.heartbeatTimer) globalThis.clearInterval(this.heartbeatTimer);
    if (this.watchdogTimer) globalThis.clearInterval(this.watchdogTimer);
    this.heartbeatTimer = null;
    this.watchdogTimer = null;
  }

  private clearConnectTimer(): void {
    if (this.connectTimer) globalThis.clearTimeout(this.connectTimer);
    this.connectTimer = null;
  }

  private stop(): void {
    this.generation += 1;
    if (this.idleDisconnectTimer) globalThis.clearTimeout(this.idleDisconnectTimer);
    this.idleDisconnectTimer = null;
    if (this.reconnectTimer) globalThis.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.clearConnectTimer();
    this.clearLiveness();
    const socket = this.socket;
    this.socket = null;
    if (socket && socket.readyState !== 3) socket.close();
    this.inboundAt = null;
    this.reconnectAttempt = 0;
    this.detachEnvironmentListeners();
  }

  private readonly handleOnline = () => {
    if (this.entries.size === 0) return;
    this.reconnectAttempt = 0;
    this.ensureConnected();
  };

  private readonly handleOffline = () => {
    this.setAllStatus('offline');
    this.socket?.close();
  };

  private readonly handleVisibility = () => {
    if (document.visibilityState === 'visible') this.ensureConnected();
  };

  private attachEnvironmentListeners(): void {
    if (this.listenersAttached) return;
    window.addEventListener('online', this.handleOnline);
    window.addEventListener('offline', this.handleOffline);
    document.addEventListener('visibilitychange', this.handleVisibility);
    this.listenersAttached = true;
  }

  private detachEnvironmentListeners(): void {
    if (!this.listenersAttached) return;
    window.removeEventListener('online', this.handleOnline);
    window.removeEventListener('offline', this.handleOffline);
    document.removeEventListener('visibilitychange', this.handleVisibility);
    this.listenersAttached = false;
  }
}

const sharedMarketTickerConnection = new MarketTickerConnectionManager();

export function subscribeMarketTicker(symbol: string, listener: TickerListener) {
  return sharedMarketTickerConnection.subscribe(symbol, listener);
}

export function resetMarketTickerConnection(): void {
  sharedMarketTickerConnection.reset();
}
