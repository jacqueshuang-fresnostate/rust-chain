import { describe, expect, it } from 'vitest';
import { emptyWithdrawalPolicy, parseWithdrawalPolicy, validWithdrawalPolicy } from './withdrawalPolicy';

describe('withdrawal policy contract', () => {
  it('keeps fresh configuration inactive without live amounts', () => {
    expect(emptyWithdrawalPolicy.enabled).toBe(false);
    expect(emptyWithdrawalPolicy.allowances).toEqual([]);
    expect(validWithdrawalPolicy(emptyWithdrawalPolicy)).toBe(true);
  });
  it('rejects malformed responses and preserves exact decimals', () => {
    expect(() => parseWithdrawalPolicy({ asset_id: 1, revision: 1 }, 1)).toThrow();
    expect(() => parseWithdrawalPolicy({ asset_id: 2, revision: 1, policy: emptyWithdrawalPolicy }, 1)).toThrow();
    const policy = { ...emptyWithdrawalPolicy, review_tiers: [
      { min_amount: '0', required_approvals: 1 },
      { min_amount: '0.000000000000000001', required_approvals: 2 }
    ] };
    expect(parseWithdrawalPolicy({ asset_id: 1, revision: 2, policy }, 1).policy).toEqual(policy);
    expect(validWithdrawalPolicy({ ...policy, security_cooling_seconds: 0 })).toBe(false);
    expect(validWithdrawalPolicy({ ...policy, review_tiers: [{ min_amount: '1', required_approvals: 2 }] })).toBe(false);
  });
  it.each(['0e9999', '1e-19', '100000000000000000000', 'NaN', 'Infinity'])('rejects unbounded policy amount %s', (value) => {
    expect(validWithdrawalPolicy({ ...emptyWithdrawalPolicy, review_tiers: [{ min_amount: value, required_approvals: 1 }] })).toBe(false);
  });
});
