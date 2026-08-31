import { describe, expect, it } from 'vitest';

import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency';

describe('financial idempotency keys', () => {
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
});
