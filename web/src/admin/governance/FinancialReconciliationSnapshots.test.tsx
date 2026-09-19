import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ApiError, ContractError } from '../../api/client';
import { authStore, type AuthSession } from '../../auth/authStore';
import { FINANCIAL_COMMAND_STORAGE_KEY } from '../../shared/idempotency';
import { FinancialReconciliationSnapshots } from './FinancialReconciliationSnapshots';
import { followupFixture, snapshotFixture } from './financialReconciliationSnapshots.fixtures';
import {
  appendSnapshotFollowup, captureSnapshot, parseFollowupHistory, parseSnapshot, parseSnapshotHistory, SNAPSHOTS_PATH
} from './financialReconciliationSnapshotsApi';

vi.mock('../../api/client', async () => ({
  ...(await vi.importActual<typeof import('../../api/client')>('../../api/client')), apiRequest: vi.fn()
}));
const access = vi.hoisted(() => ({ permissions: ['governance.financial.read', 'governance.financial.operate'], is_super_admin: false }));
vi.mock('../access', async () => ({
  ...(await vi.importActual<typeof import('../access')>('../access')), useAdminAccess: () => access
}));
const request = vi.mocked(apiRequest);
let session: AuthSession;
beforeEach(() => {
  request.mockReset();
  access.permissions = ['governance.financial.read', 'governance.financial.operate'];
  window.sessionStorage.removeItem(FINANCIAL_COMMAND_STORAGE_KEY);
  session = authStore.setSession({ accessToken: 'fixture', refreshToken: 'fixture', scope: 'admin', subject: 'admin:3' });
});
const history = () => ({ snapshots: [snapshotFixture()], total: 1, limit: 20, offset: 0 });
const followups = () => ({ snapshot_id: 41, latest_version: 1, records: [followupFixture()], total: 1, limit: 20, offset: 0 });
function mockReads() {
  request.mockImplementation(async (path) => {
    if (path.includes('/follow-ups?')) return followups();
    if (path === `${SNAPSHOTS_PATH}/41`) return snapshotFixture();
    if (path.startsWith(`${SNAPSHOTS_PATH}?`)) return history();
    throw new Error(`unexpected ${path}`);
  });
}
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  const router = createMemoryRouter([{ path: '/', element:
    <FinancialReconciliationSnapshots asset={snapshotFixture().report.asset} disabled={false} />
  }]);
  return render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
}

describe('人工采集严格证据与幂等请求', () => {
  it('保持部分覆盖、100/105 截断及原始金额文本', () => {
    const receipt = parseSnapshot(snapshotFixture(), 41, 7);
    expect(receipt.report.journal_differences).toHaveLength(100);
    expect(receipt.report.journal.imbalanced_transaction_count).toBe(105);
    expect(receipt.report.journal_differences[0].net_amount).toBe('0.000000000000000001');
  });
  it.each([
    { schema_version: 2 }, { captured_at: 'now' }, { asset_id: 8 }, { report_hash: 'invalid' },
    { idempotency_key: 'spaces forbidden' }, { reason: '' }, { precision_scale: 7 }
  ])('拒绝损坏采集元数据 %j', (change) => {
    expect(() => parseSnapshot({ ...snapshotFixture(), ...change }, 41, 7)).toThrow(ContractError);
  });
  it('拒绝伪完整覆盖、错误分页和错误跟进版本', () => {
    const value = snapshotFixture(); value.report.coverage = 'complete' as 'partial';
    expect(() => parseSnapshot(value)).toThrow(ContractError);
    expect(() => parseSnapshotHistory({ ...history(), total: 2 }, 1, 7)).toThrow(ContractError);
    expect(() => parseSnapshotHistory(history(), 2, 7)).toThrow(ContractError);
    expect(() => parseFollowupHistory({ ...followups(), latest_version: 2 }, 41, 1)).toThrow(ContractError);
    expect(() => parseFollowupHistory({ ...followups(), records: [{ ...followupFixture(), due_at: undefined }] }, 41, 1)).toThrow(ContractError);
    expect(parseFollowupHistory(followups(), 41, 1).records[0].due_at).toBeNull();
  });
  it('未知结果后跨调用重用原键，并在严格确认回执后释放', async () => {
    request.mockRejectedValueOnce(new Error('response lost'));
    await expect(captureSnapshot(session, 7, ' 人工核对 ')).rejects.toThrow();
    const originalBody = JSON.parse(String(request.mock.calls[0][1]?.body));
    request.mockImplementation(async (_, options) => ({ ...snapshotFixture(), ...JSON.parse(String(options?.body)) }));
    await captureSnapshot(session, 7, '人工核对');
    expect(JSON.parse(String(request.mock.calls[1][1]?.body))).toEqual(originalBody);
    expect(window.sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toBeNull();
  });
  it('损坏回执、错误操作者和更换会话都不能确认原请求', async () => {
    request.mockImplementation(async (_, options) => ({ ...snapshotFixture(), ...JSON.parse(String(options?.body)), admin_id: 9 }));
    await expect(captureSnapshot(session, 7, '人工核对')).rejects.toBeInstanceOf(ContractError);
    expect(window.sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).not.toBeNull();
    act(() => { authStore.setSession({ ...session, generation: 'replacement' }); });
    await expect(captureSnapshot(session, 7, '人工核对')).rejects.toThrow('会话已变化');
    expect(request).toHaveBeenCalledTimes(1);
  });
  it('跟进发送明确空值、读取版本和原始键；重放保留原回执', async () => {
    request.mockImplementation(async (_, options) => ({
      ...followupFixture(), ...JSON.parse(String(options?.body)), version: 2
    }));
    const saved = await appendSnapshotFollowup(session, snapshotFixture(), {
      expected_version: 1, owner_admin_id: null, due_at: null, notes: ' 尚未取得完整证据 '
    }, '记录核查');
    expect(saved.owner_admin_id).toBeNull();
    const body = JSON.parse(String(request.mock.calls[0][1]?.body));
    expect(body).toMatchObject({ expected_version: 1, owner_admin_id: null, due_at: null, notes: '尚未取得完整证据' });
    expect(body.idempotency_key).toBeTruthy();
  });
});

describe('人工采集与跟进 UI', () => {
  it('只读角色可看原证据及跟进，不显示采集或登记命令', async () => {
    access.permissions = ['governance.financial.read'];
    mockReads(); mount();
    expect(screen.queryByRole('button', { name: '人工采集 BTC' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '采集历史' }));
    await userEvent.click(await screen.findByRole('button', { name: '查看采集 41' }));
    expect(await screen.findByText('补充核查托管证据')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '登记跟进' })).not.toBeInTheDocument();
    expect(request.mock.calls.every(([, options]) => !options?.method)).toBe(true);
  });
  it('人工采集必须确认原因，保存后读取原证据而不是重算实时报告', async () => {
    mockReads();
    const readImpl = request.getMockImplementation()!;
    request.mockImplementation(async (path, options) => options?.method === 'POST'
      ? { ...snapshotFixture(), ...JSON.parse(String(options.body)) } : readImpl(path, options));
    mount();
    await userEvent.click(screen.getByRole('button', { name: '人工采集 BTC' }));
    expect(screen.getByRole('button', { name: '确认' })).toBeDisabled();
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '人工核对');
    await userEvent.click(screen.getByRole('button', { name: '确认' }));
    expect(await screen.findByText(/人工采集 #41 已保存/)).toBeInTheDocument();
    expect(await screen.findByText('采集 #41')).toBeInTheDocument();
    expect(request.mock.calls.some(([path]) => path === `${SNAPSHOTS_PATH}/41`)).toBe(true);
    expect(request.mock.calls.every(([path]) => path.startsWith(SNAPSHOTS_PATH))).toBe(true);
  });
  it('版本冲突保留草稿，原版本/键重试；放弃需要确认', async () => {
    mockReads();
    const readImpl = request.getMockImplementation()!;
    request.mockImplementation(async (path, options) => {
      if (options?.method === 'POST') throw new ApiError(409, 'CONFLICT', '跟进版本已变化');
      return readImpl(path, options);
    });
    mount();
    await userEvent.click(screen.getByRole('button', { name: '采集历史' }));
    await userEvent.click(await screen.findByRole('button', { name: '查看采集 41' }));
    await userEvent.click(await screen.findByRole('button', { name: '登记跟进' }));
    await userEvent.type(screen.getByRole('textbox', { name: '跟进备注' }), '核对缺失流水');
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '人工检查');
    await userEvent.click(screen.getByRole('button', { name: '追加跟进记录' }));
    expect(await screen.findByText('跟进版本已变化')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '跟进备注' })).toHaveValue('核对缺失流水');
    const first = request.mock.calls.find(([, opts]) => opts?.method === 'POST')![1]?.body;
    await userEvent.click(screen.getByRole('button', { name: '追加跟进记录' }));
    await waitFor(() => expect(request.mock.calls.filter(([, opts]) => opts?.method === 'POST')).toHaveLength(2));
    expect(request.mock.calls.filter(([, opts]) => opts?.method === 'POST')[1][1]?.body).toBe(first);
    const event = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: '取消跟进' }));
    await userEvent.click(screen.getByRole('button', { name: '继续编辑' }));
    expect(screen.getByRole('textbox', { name: '跟进备注' })).toHaveValue('核对缺失流水');
    await userEvent.click(screen.getByRole('button', { name: '取消跟进' }));
    await userEvent.click(screen.getByRole('button', { name: '放弃草稿' }));
    await waitFor(() => expect(screen.queryByRole('textbox', { name: '跟进备注' })).not.toBeInTheDocument());
  });
  it('损坏历史不能被当作空记录或授予编辑能力', async () => {
    request.mockResolvedValue({ snapshots: [], total: 10, limit: 20, offset: 0 });
    mount();
    await userEvent.click(screen.getByRole('button', { name: '采集历史' }));
    expect(await screen.findByText(/采集或跟进证据不完整/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '登记跟进' })).not.toBeInTheDocument();
  });
});
