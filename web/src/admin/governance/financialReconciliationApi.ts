import { apiRequest, ContractError } from '../../api/client';
import { canonicalDecimalText, compareDecimalText } from '../../shared/decimal';

export const RECONCILIATION_PATH = '/admin/api/v1/financial-reconciliation';
export const obligationLabels = {
  seconds_stake: '秒合约未结本金',
  seconds_conditional_payout: '秒合约逐单获胜条件赔付（含本金）',
  prediction_stake: '竞猜未结冻结本金',
  prediction_conditional_payout: '竞猜逐单获胜条件赔付',
  earn_principal: '理财未赎回本金（不含收益）',
  loan_principal_receivable: '已放款未还本金（平台应收）',
  loan_collateral: '贷款尚未释放抵押',
  margin_collateral: '杠杆在仓保证金',
  margin_recorded_interest: '杠杆在仓已记利息',
  withdrawal_reserved: '未完成提现预留（含手续费）',
  new_coin_frozen_quote: '新币人工申购冻结计价款',
  commission_pending: '已有资产快照的待结佣金',
  spot_unfilled_base: '现货未成交基础资产数量（买卖双向）'
};
export const accountLabels = { spot: '现货账户', margin: '杠杆账户' };
export const issueLabels = { mismatch: '桶余额不一致', missing_ledger: '缺少流水快照', missing_wallet: '缺少当前钱包' };

export type ReconciliationAsset = { asset_id: number; symbol: string; precision_scale: number };
export type WalletEvidence = {
  account_type: keyof typeof accountLabels;
  wallet_count: number; missing_ledger_count: number; missing_wallet_count: number; mismatch_count: number;
  available: string; frozen: string; locked: string;
  comparable_available_delta: string; comparable_frozen_delta: string; comparable_locked_delta: string;
};
export type WalletDifference = {
  account_type: keyof typeof accountLabels; user_id: number; issue: keyof typeof issueLabels;
  ledger_id: number | null;
  available: string | null; frozen: string | null; locked: string | null;
  available_after: string | null; frozen_after: string | null; locked_after: string | null;
};
export type JournalDifference = { transaction_key: string; entry_count: number; net_amount: string };
export type JournalMovement = { context: string; account_code: string; entry_count: number; net_movement: string };
export type OpenObligation = { kind: keyof typeof obligationLabels; record_count: number; amount: string };
export type ReconciliationReport = {
  asset: ReconciliationAsset;
  coverage: 'partial';
  limitations: string[];
  journal: { entry_count: number; transaction_count: number; imbalanced_transaction_count: number; first_entry_at: number | null; last_entry_at: number | null };
  journal_differences: JournalDifference[];
  journal_movements: JournalMovement[];
  journal_movement_count: number;
  wallets: WalletEvidence[];
  wallet_differences: WalletDifference[];
  wallet_difference_count: number;
  obligations: OpenObligation[];
  detail_limit: number;
};
export type FinancialReconciliation = {
  assets: ReconciliationAsset[]; total: number; limit: number; offset: number;
  report: ReconciliationReport | null; checked_at: number;
};

function invalid(): never {
  throw new ContractError('平台对账响应不完整，请刷新后重试', { path: RECONCILIATION_PATH });
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return invalid();
  return value as Record<string, unknown>;
}
function integer(value: unknown, min = 0): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min;
}
function timestamp(value: unknown): boolean {
  return integer(value, 1) && value <= 8_640_000_000_000_000;
}
function text(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}
function decimal(value: unknown): value is string {
  return typeof value === 'string' && value.length <= 100 && /^-?\d+(?:\.\d+)?$/.test(value);
}
function member(value: unknown, labels: Record<string, string>): boolean {
  return typeof value === 'string' && Object.hasOwn(labels, value);
}
function asset(value: unknown): ReconciliationAsset {
  const row = object(value);
  if (!integer(row.asset_id, 1) || !text(row.symbol) || !integer(row.precision_scale) || row.precision_scale > 18) return invalid();
  return row as ReconciliationAsset;
}
function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) return invalid();
  return value;
}
function unique(values: string[]) {
  if (new Set(values).size !== values.length) invalid();
}
function boundedDetails(value: unknown, total: number, limit: number): unknown[] {
  const rows = array(value);
  if (rows.length !== Math.min(total, limit)) return invalid();
  return rows;
}

/** 金额从不转 Number；不接收完整覆盖、伪造零值或遗漏差异页的响应。 */
export function parseFinancialReconciliation(value: unknown, selectedAsset?: number): FinancialReconciliation {
  const data = object(value);
  if (!integer(data.total) || !integer(data.limit, 1) || data.limit > 100 || !integer(data.offset)
    || data.offset > 100_000 || !timestamp(data.checked_at)) return invalid();
  const assets = array(data.assets).map(asset);
  if (assets.length !== Math.min(data.limit, Math.max(0, data.total - data.offset))) return invalid();
  unique(assets.map((row) => String(row.asset_id)));
  if (data.report === null) {
    if (selectedAsset !== undefined) return invalid();
    return data as FinancialReconciliation;
  }
  parseReconciliationReport(data.report, selectedAsset);
  return data as FinancialReconciliation;
}

/** 实时与历史证据使用同一严格合同；历史必须保持原币种、部分覆盖及截断计数。 */
export function parseReconciliationReport(value: unknown, selectedAsset?: number): ReconciliationReport {
  const report = object(value);
  const selected = asset(report.asset);
  if (selectedAsset === undefined || selected.asset_id !== selectedAsset || report.coverage !== 'partial'
    || !integer(report.detail_limit, 1) || report.detail_limit !== 100
    || !integer(report.journal_movement_count) || !integer(report.wallet_difference_count)
    || array(report.limitations).length === 0 || !array(report.limitations).every(text)) return invalid();
  const journal = object(report.journal);
  for (const key of ['entry_count', 'transaction_count', 'imbalanced_transaction_count']) {
    if (!integer(journal[key])) return invalid();
  }
  const entries = journal.entry_count as number;
  const transactions = journal.transaction_count as number;
  const imbalances = journal.imbalanced_transaction_count as number;
  if (imbalances > transactions || transactions > entries || (entries === 0) !== (transactions === 0)) return invalid();
  if (entries === 0) {
    if (journal.first_entry_at !== null || journal.last_entry_at !== null) return invalid();
  } else if (!timestamp(journal.first_entry_at) || !timestamp(journal.last_entry_at)
    || Number(journal.first_entry_at) > Number(journal.last_entry_at)) return invalid();
  const differences = boundedDetails(report.journal_differences, imbalances, report.detail_limit).map(object);
  for (const row of differences) {
    if (!text(row.transaction_key) || !integer(row.entry_count, 1) || !decimal(row.net_amount)
      || canonicalDecimalText(row.net_amount) === '0') return invalid();
  }
  unique(differences.map((row) => String(row.transaction_key)));
  const movements = boundedDetails(report.journal_movements, report.journal_movement_count, report.detail_limit).map(object);
  for (const row of movements) {
    if (!text(row.context) || !text(row.account_code) || !integer(row.entry_count, 1) || !decimal(row.net_movement)) return invalid();
  }
  unique(movements.map((row) => JSON.stringify([row.context, row.account_code])));
  if ((entries === 0) !== (report.journal_movement_count === 0)) return invalid();
  const wallets = array(report.wallets).map(object);
  if (wallets.length !== 2) return invalid();
  let walletDifferences = 0;
  for (const row of wallets) {
    if (!member(row.account_type, accountLabels)) return invalid();
    for (const field of ['wallet_count', 'missing_ledger_count', 'missing_wallet_count', 'mismatch_count']) {
      if (!integer(row[field])) return invalid();
    }
    if (Number(row.missing_ledger_count) + Number(row.mismatch_count) > Number(row.wallet_count)) return invalid();
    for (const field of ['available', 'frozen', 'locked', 'comparable_available_delta', 'comparable_frozen_delta', 'comparable_locked_delta']) {
      if (!decimal(row[field])) return invalid();
    }
    for (const field of ['available', 'frozen', 'locked']) {
      if (compareDecimalText(row[field] as string, '0') === -1
        || (row.wallet_count === 0 && canonicalDecimalText(row[field] as string) !== '0')
        || (row.mismatch_count === 0 && canonicalDecimalText(row[`comparable_${field}_delta`] as string) !== '0')) return invalid();
    }
    walletDifferences += Number(row.missing_ledger_count) + Number(row.missing_wallet_count) + Number(row.mismatch_count);
  }
  unique(wallets.map((row) => String(row.account_type)));
  if (walletDifferences !== report.wallet_difference_count) return invalid();
  const walletRows = boundedDetails(report.wallet_differences, walletDifferences, report.detail_limit).map(object);
  for (const row of walletRows) {
    if (!member(row.account_type, accountLabels) || !member(row.issue, issueLabels) || !integer(row.user_id, 1)
      || !(row.ledger_id === null || integer(row.ledger_id, 1))) return invalid();
    for (const field of ['available', 'frozen', 'locked']) {
      if (row.issue === 'missing_wallet' ? row[field] !== null : !decimal(row[field])) return invalid();
      if (row.issue === 'missing_ledger' ? row[`${field}_after`] !== null : !decimal(row[`${field}_after`])) return invalid();
    }
    if ((row.issue === 'missing_ledger') !== (row.ledger_id === null)) return invalid();
    if (row.issue === 'mismatch' && ['available', 'frozen', 'locked'].every((field) =>
      compareDecimalText(row[field] as string, row[`${field}_after`] as string) === 0)) return invalid();
  }
  unique(walletRows.map((row) => `${row.account_type}:${row.user_id}`));
  for (const scope of wallets) {
    for (const [issue, field] of Object.entries({ mismatch: 'mismatch_count', missing_ledger: 'missing_ledger_count', missing_wallet: 'missing_wallet_count' })) {
      const seen = walletRows.filter((row) => row.account_type === scope.account_type && row.issue === issue).length;
      if (seen > Number(scope[field]) || (walletDifferences <= report.detail_limit && seen !== scope[field])) return invalid();
    }
  }
  const obligations = array(report.obligations).map(object);
  if (obligations.length !== Object.keys(obligationLabels).length) return invalid();
  for (const row of obligations) {
    if (!member(row.kind, obligationLabels) || !integer(row.record_count) || !decimal(row.amount)
      || compareDecimalText(row.amount, '0') === -1
      || (row.record_count === 0 && canonicalDecimalText(row.amount) !== '0')) return invalid();
  }
  unique(obligations.map((row) => String(row.kind)));
  return report as ReconciliationReport;
}

export async function loadFinancialReconciliation(page: number, assetId?: number, signal?: AbortSignal) {
  const params = new URLSearchParams({ limit: '20', offset: String((page - 1) * 20) });
  if (assetId !== undefined) params.set('asset_id', String(assetId));
  return parseFinancialReconciliation(
    await apiRequest<unknown>(`${RECONCILIATION_PATH}?${params}`, { signal }), assetId
  );
}
