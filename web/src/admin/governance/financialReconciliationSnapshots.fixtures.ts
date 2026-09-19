import { obligationLabels, type ReconciliationReport } from './financialReconciliationApi';
import type { FollowupRecord, SnapshotDetail } from './financialReconciliationSnapshotsApi';

export function snapshotFixture(): SnapshotDetail {
  const asset = { asset_id: 7, symbol: 'BTC', precision_scale: 8 };
  const report: ReconciliationReport = {
    asset, coverage: 'partial', limitations: ['历史覆盖不完整，未补记历史。', '未核验外部托管余额。'], detail_limit: 100,
    journal: { entry_count: 105, transaction_count: 105, imbalanced_transaction_count: 105, first_entry_at: 1_789_700_000_000, last_entry_at: 1_789_700_000_000 },
    journal_differences: Array.from({ length: 100 }, (_, i) => ({ transaction_key: `tx:${i}`, entry_count: 1, net_amount: '0.000000000000000001' })),
    journal_movement_count: 1, journal_movements: [{ context: 'test', account_code: 'inventory', entry_count: 105, net_movement: '0.000000000000000105' }],
    wallets: (['spot', 'margin'] as const).map((account_type) => ({
      account_type, wallet_count: 0, mismatch_count: 0, missing_ledger_count: 0, missing_wallet_count: 0,
      available: '0', frozen: '0', locked: '0', comparable_available_delta: '0', comparable_frozen_delta: '0', comparable_locked_delta: '0'
    })),
    wallet_difference_count: 0, wallet_differences: [],
    obligations: (Object.keys(obligationLabels) as (keyof typeof obligationLabels)[]).map((kind) => ({ kind, record_count: 0, amount: '0' }))
  };
  return {
    id: 41, asset_id: 7, asset_symbol: 'BTC', precision_scale: 8, schema_version: 1,
    captured_at: 1_789_700_000_000, admin_id: 3, reason: '人工核对', idempotency_key: 'original-key',
    report_hash: 'a'.repeat(64), report
  };
}
export function followupFixture(): FollowupRecord {
  return { id: 51, snapshot_id: 41, version: 1, owner_admin_id: 3, due_at: null,
    notes: '补充核查托管证据', admin_id: 3, reason: '交接核查', recorded_at: 1_789_700_000_001, idempotency_key: 'follow-key' };
}
