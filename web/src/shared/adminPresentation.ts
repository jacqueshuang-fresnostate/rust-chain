import { ADMIN_FIELD_LABELS } from './adminFieldLabels';
import { ADMIN_FIELD_VALUE_LABELS } from './adminEnumLabels';
import { ADMIN_STATUS_META } from './adminStatus';

const statusFields = new Set(['status', 'admin_status', 'run_status', 'recovery_status', 'release_status', 'last_reload_status', 'post_listing_pair_status']);
const enabledFields = new Set(['enabled', 'active', 'registration_enabled', 'is_enabled', 'allow_early_redeem', 'allow_early_repay', 'require_fee_payment', 'post_listing_purchase_enabled']);
const aliases: Record<string, string> = {
  next_lifecycle_status: 'lifecycle_status', unlock_fee_basis: 'fee_basis', feed_runtime_status: 'runtime_status', env: 'environment',
  margin_modes: 'margin_mode', rule_scope: 'scope', document_type: 'doc_type',
  external_resolution: 'outcome', local_resolution: 'outcome',
  invalid_refund_policy: 'refund_mode', early_repay_interest_mode: 'interest_calculation_mode'
};

export function adminFieldLabel(key: string): string {
  return Object.hasOwn(ADMIN_FIELD_LABELS, key) ? ADMIN_FIELD_LABELS[key] : key;
}

/** Display boundary only: never infer a status from an arbitrary name/title/seed. */
export function adminEnumLabel(key: string, value: unknown, typedStatus = false): string | null {
  if (value === null || value === undefined || value === '' || typeof value === 'object') return null;
  const normalized = String(value).trim().toLowerCase();
  const field = aliases[key] ?? key;
  const explicit = ADMIN_FIELD_VALUE_LABELS[field]?.[normalized];
  if (typeof explicit === 'string') return explicit;
  if (typedStatus || statusFields.has(key) || enabledFields.has(key)) return ADMIN_STATUS_META[normalized]?.label ?? null;
  return null;
}
