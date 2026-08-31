import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { MarketTickerConnectionManager, type MarketTickerSnapshot } from './marketTickerSocket';

class FakeWebSocket extends EventTarget {
  readonly sent: string[] = [];
  readyState = 0;

  constructor(readonly url: string) {
    super();
  }

  open() {
    this.readyState = 1;
    this.dispatchEvent(new Event('open'));
  }

  message(value: unknown) {
    this.dispatchEvent(new MessageEvent('message', { data: typeof value === 'string' ? value : JSON.stringify(value) }));
  }

  send(value: string) {
    this.sent.push(value);
  }

  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.dispatchEvent(new Event('close'));
  }
}

function createHarness(options: ConstructorParameters<typeof MarketTickerConnectionManager>[0] = {}) {
  const sockets: FakeWebSocket[] = [];
  const manager = new MarketTickerConnectionManager({
    random: () => 0.5,
    reconnectBaseMs: 100,
    reconnectMaxMs: 800,
    ...options,
    webSocketFactory: (url) => {
      const socket = new FakeWebSocket(url);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    }
  });
  return { manager, sockets };
}

describe('MarketTickerConnectionManager', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('在所有行间复用一条连接，并按 symbol 引用计数订阅与退订', () => {
    const { manager, sockets } = createHarness();
    const btcA = vi.fn();
    const btcB = vi.fn();
    const eth = vi.fn();

    const unsubscribeBtcA = manager.subscribe('btc-usdt', btcA);
    const unsubscribeBtcB = manager.subscribe('BTC/USDT', btcB);
    const unsubscribeEth = manager.subscribe('eth_usdt', eth);

    expect(sockets).toHaveLength(1);
    expect(sockets[0].url).toBe('ws://127.0.0.1:8080/ws/public');
    sockets[0].open();
    expect(sockets[0].sent.map((message) => JSON.parse(message))).toEqual([
      { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
      { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' }
    ]);

    unsubscribeBtcA();
    expect(sockets[0].sent).toHaveLength(2);
    unsubscribeBtcB();
    expect(JSON.parse(sockets[0].sent.at(-1)!)).toEqual({ op: 'unsubscribe', channel: 'ticker', symbol: 'BTCUSDT' });
    expect(sockets[0].readyState).toBe(1);

    unsubscribeEth();
    vi.advanceTimersByTime(499);
    expect(sockets[0].readyState).toBe(1);
    const unsubscribeResubscribedBtc = manager.subscribe('BTCUSDT', vi.fn());
    expect(sockets).toHaveLength(1);
    unsubscribeResubscribedBtc();
    vi.advanceTimersByTime(500);
    expect(sockets[0].readyState).toBe(3);
  });

  it('使用连接代数忽略旧连接事件，并在连续失败时指数退避', () => {
    const { manager, sockets } = createHarness();
    const snapshots: MarketTickerSnapshot[] = [];
    const unsubscribe = manager.subscribe('BTCUSDT', (snapshot) => snapshots.push(snapshot));

    sockets[0].close();
    vi.advanceTimersByTime(99);
    expect(sockets).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(sockets).toHaveLength(2);

    sockets[1].close();
    vi.advanceTimersByTime(199);
    expect(sockets).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(sockets).toHaveLength(3);

    sockets[2].open();
    sockets[0].message({ symbol: 'BTCUSDT', last_price: '1' });
    expect(snapshots.at(-1)?.lastPrice).toBeNull();
    sockets[2].message({ symbol: 'BTCUSDT', last_price: '2', observed_at: 1234 });
    expect(snapshots.at(-1)).toMatchObject({ lastPrice: '2', observedAt: 1234, status: 'fresh' });
    sockets[0].dispatchEvent(new Event('close'));
    expect(snapshots.at(-1)).toMatchObject({ lastPrice: '2', status: 'fresh' });
    expect(sockets[2].readyState).toBe(1);

    unsubscribe();
  });

  it('从实时转为陈旧，静默超时后离线，且保留最后价格与观测时间', () => {
    let now = 0;
    const { manager, sockets } = createHarness({ freshnessMs: 1_000, inboundTimeoutMs: 2_500, now: () => now });
    const snapshots: MarketTickerSnapshot[] = [];
    const unsubscribe = manager.subscribe('BTCUSDT', (snapshot) => snapshots.push(snapshot));

    sockets[0].open();
    sockets[0].message({ symbol: 'BTCUSDT', last_price: '67890.12', observed_at: 1_735_732_800_000 });
    expect(snapshots.at(-1)).toMatchObject({
      lastPrice: '67890.12',
      observedAt: 1_735_732_800_000,
      receivedAt: 0,
      status: 'fresh'
    });

    now = 1_001;
    vi.advanceTimersByTime(500);
    expect(snapshots.at(-1)?.status).toBe('stale');

    now = 2_501;
    vi.advanceTimersByTime(500);
    expect(sockets[0].readyState).toBe(3);
    expect(snapshots.at(-1)).toMatchObject({
      lastPrice: '67890.12',
      observedAt: 1_735_732_800_000,
      receivedAt: 0,
      status: 'offline'
    });

    unsubscribe();
  });

  it('发送心跳并在新消息到达后重置静默监测起点', () => {
    let now = 0;
    const { manager, sockets } = createHarness({ heartbeatMs: 100, inboundTimeoutMs: 1_000, now: () => now });
    const unsubscribe = manager.subscribe('BTCUSDT', vi.fn());
    sockets[0].open();

    vi.advanceTimersByTime(100);
    expect(sockets[0].sent).toContain('ping');
    now = 900;
    sockets[0].message('pong');
    now = 1_500;
    vi.advanceTimersByTime(500);
    expect(sockets[0].readyState).toBe(1);

    unsubscribe();
  });

  it('连接建立超时后关闭旧 socket 并按退避创建新代', () => {
    const { manager, sockets } = createHarness({ connectTimeoutMs: 300 });
    const snapshots: MarketTickerSnapshot[] = [];
    const unsubscribe = manager.subscribe('BTCUSDT', (snapshot) => snapshots.push(snapshot));

    vi.advanceTimersByTime(300);
    expect(sockets[0].readyState).toBe(3);
    expect(snapshots.at(-1)?.status).toBe('offline');
    vi.advanceTimersByTime(100);
    expect(sockets).toHaveLength(2);

    unsubscribe();
  });

  it('忽略非法 Decimal 价格和非安全观测时间', () => {
    const { manager, sockets } = createHarness();
    const snapshots: MarketTickerSnapshot[] = [];
    const unsubscribe = manager.subscribe('BTCUSDT', (snapshot) => snapshots.push(snapshot));
    sockets[0].open();

    sockets[0].message({ symbol: 'BTCUSDT', last_price: 'not-a-price', observed_at: 1234 });
    expect(snapshots.at(-1)?.lastPrice).toBeNull();
    sockets[0].message({ symbol: 'BTCUSDT', last_price: '25.5000', observed_at: Number.MAX_SAFE_INTEGER + 1 });
    expect(snapshots.at(-1)).toMatchObject({ lastPrice: '25.5', status: 'fresh' });
    expect(snapshots.at(-1)?.observedAt).toBeUndefined();

    unsubscribe();
  });
});
