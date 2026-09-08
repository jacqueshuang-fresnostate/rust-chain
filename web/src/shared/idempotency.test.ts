import { describe, expect, it } from 'vitest';

import {
  canonicalRequestIntent,
  FinancialCommandIntentStore,
  RetryStableIdempotencyKeys,
  runRecoverableFinancialCommand
} from './idempotency';

describe('financial idempotency keys', () => {
  it('finds only existing equivalent scoped intents without allocating a key', () => {
    sessionStorage.clear();
    const store = new FinancialCommandIntentStore({ storage: sessionStorage });
    const scope = { assetId: 12, authScope: 'admin' as const, command: 'recharge', generation: 'session-a', subject: 'admin:7', userId: 42 };
    const values = { amount: '1.2300', reason: 'manual' };
    expect(store.hasPending(scope, values)).toBe(false);
    expect(sessionStorage.length).toBe(0);
    const lease = store.acquire(scope, values);
    const before = sessionStorage.getItem('exchange_admin_financial_command_intents_v1');
    expect(store.hasPending(scope, { amount: '1.23', reason: ' manual ' })).toBe(true);
    expect(store.hasPending(scope, { ...values, reason: 'different' })).toBe(false);
    expect(store.hasPending({ ...scope, generation: 'other' }, values)).toBe(false);
    expect(store.hasPending({ ...scope, assetId: 13 }, values)).toBe(false);
    expect(sessionStorage.getItem('exchange_admin_financial_command_intents_v1')).toBe(before);
    store.complete(lease);
    expect(store.hasPending(scope, values)).toBe(false);
  });
  it('reuses a key through retries and rotates it after success', () => {
    let sequence = 0;
    const keys = new RetryStableIdempotencyKeys('admin-test', (prefix) => `${prefix}-${++sequence}`);
    const intent = canonicalRequestIntent({ amount: '25.5', asset_id: 12, reason: ' manual ', user_id: 7 });
    const equivalent = canonicalRequestIntent({ user_id: 7, reason: 'manual', asset_id: 12, amount: '25.5' });

    const firstKey = keys.acquire(intent);
    expect(keys.acquire(equivalent)).toBe(firstKey);
    keys.complete(intent, firstKey);
    expect(keys.acquire(intent)).not.toBe(firstKey);
  });

  it('treats equivalent decimal text as one recoverable command across reloads', () => {
    sessionStorage.clear();
    let sequence = 0;
    const createStore = () => new FinancialCommandIntentStore({
      keyFactory: (prefix) => `${prefix}-${++sequence}`,
      prefix: 'admin-recharge',
      storage: sessionStorage
    });
    const scope = { assetId: 12, authScope: 'admin' as const, command: 'recharge', generation: 'session-a', subject: 'admin:7', userId: 42 };
    const first = createStore().acquire(scope, { amount: '25.50', reason: ' manual ' });
    const reloadedStore = createStore();
    const recovered = reloadedStore.acquire(scope, { amount: '25.5', reason: 'manual' });

    expect(recovered.key).toBe(first.key);
    reloadedStore.markUncertain(recovered);
    expect(createStore().acquire(scope, { amount: '25.500000000000000000', reason: 'manual' }).key).toBe(first.key);
    expect(createStore().acquire({ ...scope, generation: 'session-b' }, { amount: '25.5', reason: 'manual' }).key).not.toBe(first.key);
  });

  it('keeps an unresolved command across arbitrary elapsed time and rotates only after intent change', () => {
    sessionStorage.clear();
    let now = 0;
    let sequence = 0;
    const store = new FinancialCommandIntentStore({
      keyFactory: (prefix) => `${prefix}-${++sequence}`,
      now: () => now,
      prefix: 'admin-recharge',
      storage: sessionStorage
    });
    const scope = { assetId: 12, authScope: 'admin' as const, command: 'recharge', generation: 'session-a', subject: 'admin:7', userId: 42 };
    const first = store.acquire(scope, { amount: '25.50', reason: 'manual' });
    store.markUncertain(first);
    now = 365 * 24 * 60 * 60 * 1_000;

    expect(store.acquire(scope, { amount: '25.5', reason: 'manual' }).key).toBe(first.key);
    expect(store.acquire(scope, { amount: '25.6', reason: 'manual' }).key).not.toBe(first.key);
  });

  it('deduplicates commit-before-timeout and response-drop retries after reload/remount', async () => {
    sessionStorage.clear();
    let sequence = 0;
    let effects = 0;
    const completedKeys = new Set<string>();
    const scope = { assetId: 12, authScope: 'admin' as const, command: 'recharge', generation: 'session-a', subject: 'admin:7', userId: 42 };
    const values = { amount: '25.50', reason: 'manual' };
    const createStore = () =>
      new FinancialCommandIntentStore({
        keyFactory: (prefix) => `${prefix}-${++sequence}`,
        prefix: 'admin-recharge',
        storage: sessionStorage
      });
    const server = async (key: string, dropResponse: boolean) => {
      if (!completedKeys.has(key)) {
        completedKeys.add(key);
        effects += 1;
      }
      if (dropResponse) throw new Error('响应已丢失');
      return { rechargeId: 'recharge-1' };
    };

    await expect(
      runRecoverableFinancialCommand({ request: (key) => server(key, true), scope, store: createStore(), values })
    ).rejects.toThrow('响应已丢失');

    // 新 store 模拟组件重挂载/页面 reload；等值文本仍重放同一服务端命令。
    await expect(
      runRecoverableFinancialCommand({
        request: (key) => server(key, false),
        scope,
        store: createStore(),
        values: { amount: '25.5', reason: 'manual' }
      })
    ).resolves.toEqual({ rechargeId: 'recharge-1' });
    expect(effects).toBe(1);

    const next = createStore().acquire(scope, values);
    expect(next.key).not.toBe([...completedKeys][0]);
  });

  it('releases a key only for an explicitly classified server rejection', async () => {
    sessionStorage.clear();
    let sequence = 0;
    const store = new FinancialCommandIntentStore({
      keyFactory: (prefix) => `${prefix}-${++sequence}`,
      prefix: 'admin-recharge',
      storage: sessionStorage
    });
    const scope = { assetId: 12, authScope: 'admin' as const, command: 'recharge', generation: 'session-a', subject: 'admin:7', userId: 42 };
    const values = { amount: '25.5', reason: 'manual' };
    let rejectedKey = '';

    await expect(
      runRecoverableFinancialCommand({
        isDefinitiveFailure: () => true,
        request: async (key) => {
          rejectedKey = key;
          throw new Error('服务端明确拒绝');
        },
        scope,
        store,
        values
      })
    ).rejects.toThrow('服务端明确拒绝');

    expect(store.acquire(scope, values).key).not.toBe(rejectedKey);
  });
});
