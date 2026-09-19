import { compareDecimalText, decimalFitsStorage } from '../../shared/decimal';

export type WithdrawalPolicy = {
  enabled: boolean;
  amount_basis: 'principal' | 'total_reserved';
  address_cooling_seconds: number | null;
  security_cooling_seconds: number | null;
  allowances: Array<{
    user_id: number | null;
    kyc_level: number | null;
    window: 'rolling';
    window_seconds: number;
    pending_mode: 'all_outstanding' | 'within_window';
    max_amount: string;
  }>;
  review_tiers: Array<{ min_amount: string; required_approvals: number }>;
};

export type WithdrawalPolicyResponse = {
  asset_id: number;
  revision: number;
  policy: WithdrawalPolicy;
};

export const emptyWithdrawalPolicy: WithdrawalPolicy = {
  enabled: false, amount_basis: 'principal', address_cooling_seconds: null,
  security_cooling_seconds: null, allowances: [], review_tiers: []
};

function integer(value: unknown, min: number, max = 4294967295): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max;
}

function amount(value: unknown): value is string {
  return decimalFitsStorage(value) && compareDecimalText(value, '0') !== -1;
}

function object(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

export function validWithdrawalPolicy(value: unknown): value is WithdrawalPolicy {
  if (!object(value) || typeof value.enabled !== 'boolean' ||
    !['principal', 'total_reserved'].includes(String(value.amount_basis)) ||
    ![value.address_cooling_seconds, value.security_cooling_seconds].every((v) => v === null || integer(v, 1)) ||
    !Array.isArray(value.allowances) || !Array.isArray(value.review_tiers) ||
    value.allowances.length > 100 || value.review_tiers.length > 50) return false;
  if (!value.allowances.every((rule: unknown) => object(rule) &&
    (rule.user_id === null || integer(rule.user_id, 1, Number.MAX_SAFE_INTEGER)) &&
    (rule.kyc_level === null || integer(rule.kyc_level, 0, 2147483647)) &&
    rule.window === 'rolling' && integer(rule.window_seconds, 1) &&
    ['all_outstanding', 'within_window'].includes(String(rule.pending_mode)) && amount(rule.max_amount))) return false;
  return value.review_tiers.every((tier: unknown, index: number, tiers: unknown[]) => {
    if (!object(tier) || !amount(tier.min_amount) || !integer(tier.required_approvals, 1, 20)) return false;
    if (index === 0) return compareDecimalText(tier.min_amount, '0') === 0;
    const previous = tiers[index - 1] as WithdrawalPolicy['review_tiers'][number];
    return compareDecimalText(tier.min_amount, previous.min_amount) === 1 &&
      tier.required_approvals >= previous.required_approvals;
  });
}

export function parseWithdrawalPolicy(value: unknown, assetId: number): WithdrawalPolicyResponse {
  if (!object(value) || value.asset_id !== assetId || !integer(value.revision, 0, Number.MAX_SAFE_INTEGER) ||
    !validWithdrawalPolicy(value.policy)) throw new Error('提现策略响应格式无效');
  return value as WithdrawalPolicyResponse;
}
