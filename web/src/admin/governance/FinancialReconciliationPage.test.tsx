import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ApiError, ContractError } from '../../api/client';
import { FinancialReconciliationPage } from './FinancialReconciliationPage';
import {
  type FinancialReconciliation, type ReconciliationReport, obligationLabels,
  parseFinancialReconciliation, RECONCILIATION_PATH
} from './financialReconciliationApi';

vi.mock('../../api/client', async () => ({
  ...(await vi.importActual<typeof import('../../api/client')>('../../api/client')),
  apiRequest: vi.fn()
}));
const request = vi.mocked(apiRequest);
vi.mock('../access', async () => ({
  ...(await vi.importActual<typeof import('../access')>('../access')),
  useAdminAccess: () => ({ permissions: ['governance.financial.read'], is_super_admin: false })
}));
const asset = { asset_id: 7, symbol: 'BTC', precision_scale: 8 };
function fixture(selected = true): FinancialReconciliation {
  const report: ReconciliationReport = {
    asset, coverage: 'partial', limitations: ['未补记历史；首次分录不是完整覆盖起点。', '库存净变动不是实际托管库存。'],
    detail_limit: 100,
    journal: { entry_count: 2, transaction_count: 2, imbalanced_transaction_count: 2, first_entry_at: 1_789_700_000_000, last_entry_at: 1_789_700_000_001 },
    journal_differences: [
      { transaction_key: 'opposite:a', entry_count: 1, net_amount: '0.000000000000000001' },
      { transaction_key: 'opposite:b', entry_count: 1, net_amount: '-0.000000000000000001' }
    ],
    journal_movements: [{ context: 'convert', account_code: 'platform_convert_inventory', entry_count: 2, net_movement: '0.000000000000000000' }],
    journal_movement_count: 1,
    wallets: ['spot', 'margin'].map((scope) => ({
      account_type: scope as 'spot' | 'margin', wallet_count: scope === 'spot' ? 1 : 0,
      missing_ledger_count: 0, missing_wallet_count: 0, mismatch_count: scope === 'spot' ? 1 : 0,
      available: scope === 'spot' ? '123456789.123456789012345678' : '0', frozen: '0', locked: '0',
      comparable_available_delta: scope === 'spot' ? '1' : '0', comparable_frozen_delta: '0', comparable_locked_delta: '0'
    })),
    wallet_differences: [{
      account_type: 'spot', user_id: 8, ledger_id: 9, issue: 'mismatch',
      available: '2', frozen: '0', locked: '0', available_after: '1', frozen_after: '0', locked_after: '0'
    }],
    wallet_difference_count: 1,
    obligations: Object.keys(obligationLabels).map((kind) => ({
      kind: kind as keyof typeof obligationLabels, record_count: 0, amount: '0'
    }))
  };
  return { assets: [asset], total: 1, limit: 20, offset: 0, report: selected ? report : null, checked_at: 1_789_700_000_000 };
}
function mount() {
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    <FinancialReconciliationPage />
  </QueryClientProvider>);
}
beforeEach(() => { request.mockReset(); });

describe('平台对账严格响应', () => {
  it('保留极小精度、资产身份和不完整覆盖，互相抵消的交易仍各自可见', () => {
    const data = parseFinancialReconciliation(fixture(), 7);
    expect(data.report?.journal_differences[0].net_amount).toBe('0.000000000000000001');
    expect(data.report?.wallets[0].available).toBe('123456789.123456789012345678');
    expect(data.report?.journal.imbalanced_transaction_count).toBe(2);
    expect(data.report?.coverage).toBe('partial');
  });
  it.each([
    (r: ReconciliationReport) => { r.coverage = 'complete' as 'partial'; },
    (r: ReconciliationReport) => { r.asset.asset_id = 8; },
    (r: ReconciliationReport) => { r.asset.precision_scale = 19; },
    (r: ReconciliationReport) => { r.limitations = []; },
    (r: ReconciliationReport) => { r.journal.entry_count = -1; },
    (r: ReconciliationReport) => { r.journal.first_entry_at = null; },
    (r: ReconciliationReport) => { r.journal.imbalanced_transaction_count = 0; },
    (r: ReconciliationReport) => { r.journal_differences[0].net_amount = '0'; },
    (r: ReconciliationReport) => { r.journal_differences[0].net_amount = 1 as unknown as string; },
    (r: ReconciliationReport) => { r.journal_differences.push(r.journal_differences[0]); },
    (r: ReconciliationReport) => { r.journal_movement_count = 0; },
    (r: ReconciliationReport) => { r.wallets.pop(); },
    (r: ReconciliationReport) => { r.wallet_difference_count = 0; },
    (r: ReconciliationReport) => { r.wallet_differences[0].available_after = '2'; },
    (r: ReconciliationReport) => { r.wallet_differences[0].ledger_id = null; },
    (r: ReconciliationReport) => { r.obligations[0].kind = 'made-up' as 'seconds_stake'; },
    (r: ReconciliationReport) => { r.obligations[0].amount = '-1'; },
    (r: ReconciliationReport) => { r.obligations[0].amount = '1'; },
    (r: ReconciliationReport) => { r.obligations.pop(); }
  ])('拒绝缺失、伪完整和矛盾证据 %#', (mutate) => {
    const data = structuredClone(fixture());
    mutate(data.report!);
    expect(() => parseFinancialReconciliation(data, 7)).toThrow(ContractError);
  });
  it('未选择时不得带报告，选择后不得用空报告或其他资产代替', () => {
    expect(() => parseFinancialReconciliation(fixture())).toThrow(ContractError);
    expect(() => parseFinancialReconciliation(fixture(false), 7)).toThrow(ContractError);
    expect(() => parseFinancialReconciliation({ ...fixture(false), checked_at: 'now' })).toThrow(ContractError);
    expect(parseFinancialReconciliation(fixture(false)).report).toBeNull();
  });
  it('缺失一侧保留 null，有界明细保留全部差异计数', () => {
    const data = structuredClone(fixture());
    const report = data.report!;
    report.wallets[0].mismatch_count = 0;
    report.wallets[0].missing_ledger_count = 1;
    report.wallets[0].comparable_available_delta = '0';
    Object.assign(report.wallet_differences[0], {
      issue: 'missing_ledger', ledger_id: null, available_after: null, frozen_after: null, locked_after: null
    });
    report.journal = { ...report.journal, entry_count: 105, transaction_count: 105, imbalanced_transaction_count: 105 };
    report.journal_differences = Array.from({ length: 100 }, (_, i) => ({ transaction_key: `tx:${i}`, entry_count: 1, net_amount: '1' }));
    expect(parseFinancialReconciliation(data, 7).report?.journal.imbalanced_transaction_count).toBe(105);
    report.wallet_differences[0].available_after = '0';
    expect(() => parseFinancialReconciliation(data, 7)).toThrow(ContractError);
  });
});

describe('只读平台对账工作台', () => {
  it('先选择币种，只发 GET，五个证据视图均显示口径且没有资金动作', async () => {
    request.mockImplementation(async (path) => fixture(path.includes('asset_id=7')));
    mount();
    await userEvent.click(await screen.findByRole('button', { name: '查看 BTC 对账' }));
    expect(await screen.findByText('异常账户 1 个')).toBeInTheDocument();
    expect(request).toHaveBeenLastCalledWith(`${RECONCILIATION_PATH}?limit=20&offset=0&asset_id=7`, expect.anything());
    await userEvent.click(screen.getByRole('tab', { name: '分录差异' }));
    expect(await screen.findByText('不平衡交易 2 个')).toBeInTheDocument();
    expect(screen.getByText('opposite:a')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('tab', { name: '科目净变动' }));
    expect(await screen.findByText('platform_convert_inventory')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('tab', { name: '未结义务' }));
    expect(await screen.findByText('理财未赎回本金（不含收益）')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('tab', { name: '覆盖边界' }));
    expect(await screen.findByText('未补记历史；首次分录不是完整覆盖起点。')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /补账|结算|修复|确认/ })).not.toBeInTheDocument();
    expect(request.mock.calls.every(([, options]) => !options?.method || options.method === 'GET')).toBe(true);
  });
  it('刷新失败保留旧快照并显式提示，不显示新的正常结论', async () => {
    request.mockImplementation(async (path) => fixture(path.includes('asset_id=7')));
    mount();
    await userEvent.click(await screen.findByRole('button', { name: '查看 BTC 对账' }));
    await screen.findByText('异常账户 1 个');
    request.mockRejectedValue(new ApiError(500, 'INTERNAL_ERROR', '快照读取失败'));
    await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    expect(await screen.findByText(/保留上次快照/)).toBeInTheDocument();
    expect(screen.getByText('异常账户 1 个')).toBeInTheDocument();
  });
  it('目录分页使已加载的资产选择失效', async () => {
    let resolveOld: (value: unknown) => void = () => undefined;
    const first = { ...fixture(false), total: 21, assets: Array.from({ length: 20 }, (_, i) => ({ ...asset, asset_id: i + 1, symbol: `A${i + 1}` })) };
    request.mockResolvedValue(first);
    mount();
    await screen.findByText('A1');
    request.mockImplementation(() => new Promise((resolve) => { resolveOld = resolve; }));
    await userEvent.click(screen.getByRole('button', { name: '查看 A7 对账' }));
    await act(async () => resolveOld({ ...first, report: fixture().report }));
    await screen.findByText('异常账户 1 个');
    request.mockResolvedValue({ ...fixture(false), assets: [{ ...asset, asset_id: 21, symbol: 'LAST' }], total: 21, offset: 20 });
    await userEvent.click(screen.getByText('2', { selector: '.semi-page-item' }));
    expect(await screen.findByText('尚未选择对账资产')).toBeInTheDocument();
    await waitFor(() => expect(request).toHaveBeenLastCalledWith(`${RECONCILIATION_PATH}?limit=20&offset=20`, expect.anything()));
  });
  it('空目录或契约故障不是完整覆盖', async () => {
    request.mockResolvedValue({ ...fixture(false), total: 0, assets: [] });
    mount();
    expect(await screen.findByText('暂无资产')).toBeInTheDocument();
    expect(screen.getByText(/历史覆盖不完整。本页不是/)).toBeInTheDocument();
  });
});
