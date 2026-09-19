import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ApiError, ContractError } from '../../api/client';
import { AdminAccessProvider, adminPermissionForRequest, adminReadPermissionForPath } from '../access';
import { adminRoutes } from '../routes';
import { FinancialRetriesPage, FinancialRetrySecondsOrderPage } from './FinancialRetriesPage';
import {
  FINANCIAL_RETRIES_PATH as root, type FinancialRetries, type FinancialRetry,
  parseFinancialRetries, requeueFinancialRetry, parseFinancialRetryIncident, loadFinancialRetrySecondsOrder
} from './financialRetriesApi';

vi.mock('../../api/client', async () => ({
  ...(await vi.importActual<typeof import('../../api/client')>('../../api/client')),
  apiRequest: vi.fn()
}));
const request = vi.mocked(apiRequest);

function row(overrides: Partial<FinancialRetry> = {}): FinancialRetry {
  return {
    version: 'a'.repeat(64),
    task_kind: 'earn', item_id: 7, outcome: 'failed', attempt_count: 3,
    last_attempt_at: 1_789_700_000_000, next_attempt_at: 1_789_800_000_000,
    lease_status: 'none', source_type: 'earn_subscription', source_order_id: '90071992547409999',
    user_id: 8, amount: '123.123456789012345678', asset: 'BTC',
    incident: { version: 0, owner_admin_id: null, owner_name: null, due_at: null, updated_by: null, updated_at: null },
    failure_code: null, failure_at: null, review_window_start: null, review_window_end: null, ...overrides
  };
}
function data(overrides: Partial<FinancialRetry> = {}): FinancialRetries {
  const item = row(overrides);
  return {
    retries: [item], total: 1, counts: [{ outcome: item.outcome, count: 1 }],
    limit: 20, offset: 0, checked_at: 1_789_700_000_000
  };
}
function mount(
  permissions = ['governance.financial.read'],
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
) {
  const router = createMemoryRouter([{ path: '/', element:
    <AdminAccessProvider access={{
      admin_id: 1, username: 'test', role_id: 2, role_name: 'test', permissions, is_super_admin: false
    }}><FinancialRetriesPage /></AdminAccessProvider>
  }]);
  return render(<QueryClientProvider client={client}>
    <RouterProvider router={router} />
  </QueryClientProvider>);
}
beforeEach(() => { request.mockReset(); });

describe('资金重试严格契约及权限', () => {
  it('保留金额文本、来源标识和服务端租约，不隐式发起任何写入', () => {
    const parsed = parseFinancialRetries(data());
    expect(parsed.retries[0].amount).toBe('123.123456789012345678');
    expect(parsed.retries[0].source_order_id).toBe('90071992547409999');
    expect(request).not.toHaveBeenCalled();
  });
  it.each([
    { total: -1 }, { total: 2 }, { checked_at: 'now' }, { counts: [] },
    { counts: [{ outcome: 'failed', count: 1 }, { outcome: 'failed', count: 1 }] },
    { retries: [row({ amount: 10 as unknown as string })] },
    { retries: [row({ lease_status: 'unknown' as 'none' })] },
    { retries: [row({ task_kind: 'wallet' as 'earn' })] },
    { retries: [row({ user_id: undefined })] },
    { retries: [row({ last_attempt_at: undefined })] },
    { retries: [row({ source_order_id: undefined })] }
  ])('响应不完整时失败关闭 %j', (changes) => {
    expect(() => parseFinancialRetries({ ...data(), ...changes })).toThrow(ContractError);
  });
  it('精确映射页面和实际方法，对伪造后缀、类型、ID 失败关闭', () => {
    expect(adminReadPermissionForPath('/admin/governance/financial-retries')).toBe('governance.financial.read');
    expect(adminReadPermissionForPath('/admin/governance/financial-retries/extra')).toBe('admin.unmapped.read');
    expect(adminPermissionForRequest(root, 'GET')).toBe('governance.financial.read');
    expect(adminPermissionForRequest(`${root}/earn/1/requeue`, 'POST')).toBe('governance.financial.operate');
    for (const path of [
      `${root}/earn/0/requeue`, `${root}/earn/01/requeue`, `${root}/earn/+1/requeue`,
      `${root}/earn/18446744073709551616/requeue`, `${root}/wallet/1/requeue`,
      `${root}/earn/1/requeue/extra`, `${root}/earn//1/requeue`, `${root}-export`, root
    ]) {
      expect(adminPermissionForRequest(path, 'POST')).toBe('admin.unmapped.write');
    }
    expect(adminPermissionForRequest(`${root}/earn/1/requeue`, 'GET')).toBe('admin.unmapped.read');
    expect(adminRoutes.find((route) => route.path === 'governance/financial-retries')?.handle).toEqual({ permission: 'governance.financial.read' });
  });
  it('变更响应身份不匹配时失败关闭', async () => {
    request.mockResolvedValue(row({ item_id: 8 }));
    await expect(requeueFinancialRetry(row(), '核查')).rejects.toThrow(ContractError);
  });
  it.each([
    { version: 'a'.repeat(64) }, { outcome: 'running' as const },
    { lease_status: 'active' as const }, { lease_status: 'expired' as const },
    { attempt_count: 4 }, { last_attempt_at: 1_789_700_000_001 },
    { task_kind: 'loan' as const }
  ])('拒绝未提交的新版本或违反调度原子契约的回执 %j', async (changes) => {
    request.mockResolvedValue(row({ version: 'b'.repeat(64), outcome: 'ready', ...changes }));
    await expect(requeueFinancialRetry(row(), '核查')).rejects.toThrow(ContractError);
    expect(request).toHaveBeenCalledTimes(1);
  });
  it.each([
    { task_kind: 'seconds' as const }, { lease_status: 'active' as const }
  ])('秒合约与有效租约即使绕过界面也不能提交 %j', async (changes) => {
    await expect(requeueFinancialRetry(row(changes), '核查')).rejects.toThrow(ContractError);
    expect(request).not.toHaveBeenCalled();
  });
  it.each(['0', '01', '+1', '7/extra', '9007199254740992'])('原订单深链拒绝无效 ID %s 且不发请求', async (id) => {
    await expect(loadFinancialRetrySecondsOrder(id)).rejects.toThrow(ContractError);
    expect(request).not.toHaveBeenCalled();
  });
  it('原订单深链拒绝返回其他订单，保留金额原文', async () => {
    const order = { id: 7, user_id: 8, status: 'manual_review', stake_amount: '1.123456789012345678' };
    request.mockResolvedValueOnce({ ...order, id: 8 }).mockResolvedValueOnce(order);
    await expect(loadFinancialRetrySecondsOrder('7')).rejects.toThrow(ContractError);
    await expect(loadFinancialRetrySecondsOrder('7')).resolves.toEqual(order);
    expect(adminReadPermissionForPath('/admin/seconds-contract/orders/7')).toBe('seconds.orders.read');
    expect(adminRoutes.find((route) => route.path === 'seconds-contract/orders/:orderId')?.handle).toEqual({ permission: 'seconds.orders.read' });
  });
  it('佣金冲正复用既有结算权限并拒绝畸形路径和错误方法', () => {
    expect(adminPermissionForRequest('/admin/api/v1/agent-commissions/7/reversal', 'POST')).toBe('agents.commissions.settle');
    for (const path of ['/agent-commissions/0/reversal', '/agent-commissions/01/reversal', '/agent-commissions/+1/reversal', '/agent-commissions/7/reversal/extra', '/agent-commissions/18446744073709551616/reversal']) {
      expect(adminPermissionForRequest(path, 'POST')).toBe('admin.unmapped.write');
    }
    expect(adminPermissionForRequest('/agent-commissions/7/reversal', 'GET')).toBe('admin.unmapped.read');
    expect(adminPermissionForRequest('/agent-commissions/7/reversal', 'PATCH')).toBe('admin.unmapped.write');
  });
  it('元数据精确映射 PATCH；秒合约排期始终未登记，E02 复用只读权限', () => {
    for (const kind of ['earn', 'loan', 'commission', 'seconds']) {
      expect(adminPermissionForRequest(`${root}/${kind}/1/incident`, 'PATCH')).toBe('governance.financial.operate');
      expect(adminPermissionForRequest(`${root}/${kind}/1/incident`, 'POST')).toBe('admin.unmapped.write');
    }
    for (const path of [`${root}/seconds/1/requeue`, `${root}/seconds/0/incident`, `${root}/seconds/01/incident`, `${root}/seconds/1/incident/extra`]) {
      expect(adminPermissionForRequest(path, 'PATCH')).toBe('admin.unmapped.write');
    }
    expect(adminPermissionForRequest(`${root}/seconds/1/requeue`, 'POST')).toBe('admin.unmapped.write');
    expect(adminPermissionForRequest('/admin/api/v1/financial-reconciliation?asset_id=1', 'GET')).toBe('governance.financial.read');
    expect(adminPermissionForRequest('/financial-reconciliation', 'POST')).toBe('admin.unmapped.write');
    expect(adminPermissionForRequest('/financial-reconciliation/extra', 'GET')).toBe('admin.unmapped.read');
    expect(adminReadPermissionForPath('/admin/financial-reconciliation')).toBe('governance.financial.read');
    expect(adminRoutes.find((route) => route.path === 'financial-reconciliation')?.handle).toEqual({ permission: 'governance.financial.read' });
  });
  it('元数据版本、空字段和编辑者证据缺失时失败关闭', () => {
    for (const change of [{ version: -1 }, { due_at: undefined }, { owner_name: 'ghost' }, { version: 1 }, { owner_admin_id: 0 }]) {
      expect(() => parseFinancialRetryIncident({ ...row().incident, ...change })).toThrow(ContractError);
    }
  });
});

describe('资金异常工作台', () => {
  it('只读角色只发 GET，显示中文分类/计数并刷新，不暴露排期操作', async () => {
    request.mockResolvedValue(data());
    mount();
    await screen.findByText('筛选结果 1 条');
    expect(screen.getByText('理财到期赎回')).toBeInTheDocument();
    expect(screen.getByText('执行失败 1 条')).toBeInTheDocument();
    expect(screen.getByText('90071992547409999')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /重新排期/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
    expect(request.mock.calls.every(([, options]) => !options?.method || options.method === 'GET')).toBe(true);
  });
  it('有效租约禁用排期，过期租约允许排期，原因必填且不发送资金字段', async () => {
    request.mockResolvedValue(data({ lease_status: 'active', outcome: 'running' }));
    mount(['governance.financial.read', 'governance.financial.operate']);
    const action = await screen.findByRole('button', { name: '重新排期 理财到期赎回 7' });
    expect(action).toBeDisabled();
    request.mockResolvedValue(data({ lease_status: 'expired', outcome: 'running' }));
    await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(action).toBeEnabled());
    await userEvent.click(action);
    const confirm = await screen.findByRole('button', { name: '确认' });
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '  核查完成  ');
    request.mockImplementation(async (_path, options) => options?.method === 'POST'
      ? row({ version: 'b'.repeat(64), outcome: 'ready', lease_status: 'none' }) : data({ version: 'b'.repeat(64), outcome: 'ready' }));
    await userEvent.click(confirm);
    await screen.findByText('重新排期已记录，等待后台任务扫描。');
    expect(request).toHaveBeenCalledWith(`${root}/earn/7/requeue`, { method: 'POST', body: JSON.stringify({ reason: '核查完成', expected_version: 'a'.repeat(64) }) });
    expect(request.mock.calls.filter(([, options]) => options?.method === 'POST')).toHaveLength(1);
  });
  it('无效成功回执保留原因，不显示成功、不自动重发', async () => {
    request.mockResolvedValue(data());
    mount(['governance.financial.read', 'governance.financial.operate']);
    await userEvent.click(await screen.findByRole('button', { name: '重新排期 理财到期赎回 7' }));
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '核对排期回执');
    request.mockResolvedValue(row({ outcome: 'ready' }));
    await userEvent.click(screen.getByRole('button', { name: '确认' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('资金异常响应不完整');
    expect(screen.getByRole('textbox', { name: '操作原因' })).toHaveValue('核对排期回执');
    expect(screen.queryByText('重新排期已记录，等待后台任务扫描。')).not.toBeInTheDocument();
    expect(request).toHaveBeenCalledTimes(2);
  });
  it('确认期间版本变化且被 worker 领取后，旧确认失效，不改发新版本命令', async () => {
    request.mockResolvedValue(data());
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    mount(['governance.financial.read', 'governance.financial.operate'], client);
    await userEvent.click(await screen.findByRole('button', { name: '重新排期 理财到期赎回 7' }));
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '旧快照核查');
    await act(async () => {
      client.setQueriesData({ queryKey: ['admin-financial-retries'] },
        data({ version: 'b'.repeat(64), lease_status: 'active', outcome: 'running' }));
    });
    await waitFor(() => expect(screen.queryByRole('textbox', { name: '操作原因' })).not.toBeInTheDocument());
    expect(screen.getByRole('button', { name: '重新排期 理财到期赎回 7' })).toBeDisabled();
    expect(request).toHaveBeenCalledTimes(1);
  });
  it('冲突保留原因，不自动重试写入；刷新失败保留快照但禁用操作', async () => {
    request.mockResolvedValue(data());
    mount(['governance.financial.read', 'governance.financial.operate']);
    await userEvent.click(await screen.findByRole('button', { name: '重新排期 理财到期赎回 7' }));
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '等待租约后核查');
    request.mockRejectedValue(new ApiError(409, 'CONFLICT', '任务租约仍有效'));
    await userEvent.click(screen.getByRole('button', { name: '确认' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('任务租约仍有效');
    await waitFor(() => expect(screen.getByRole('textbox', { name: '操作原因' })).toHaveValue('等待租约后核查'));
    expect(request.mock.calls.filter(([, options]) => options?.method === 'POST')).toHaveLength(1);
    await userEvent.click(screen.getByRole('button', { name: '取消' }));
    let rejectRefresh: (error: Error) => void = () => undefined;
    request.mockImplementation(() => new Promise((_, reject) => { rejectRefresh = reject; }));
    await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    await act(async () => { rejectRefresh(new ApiError(500, 'INTERNAL_ERROR', '读取失败')); });
    expect(await screen.findByRole('alert')).toHaveTextContent('读取失败');
    await waitFor(() => expect(screen.getByRole('button', { name: '重新排期 理财到期赎回 7' })).toBeDisabled());
    expect(screen.getByText('筛选结果 1 条')).toBeInTheDocument();
  });
  it('空列表显示空状态且没有写入', async () => {
    request.mockResolvedValue({ ...data(), total: 0, retries: [], counts: [] });
    mount();
    await screen.findByText('暂无符合条件的资金重试记录');
    expect(request).toHaveBeenCalledTimes(1);
  });
  it('分页发送偏移，筛选后回到第一页且提交原始枚举', async () => {
    request.mockResolvedValue({ ...data(), total: 21, retries: Array.from({ length: 20 }, (_, index) => row({ item_id: index + 1 })), counts: [{ outcome: 'failed', count: 21 }] });
    mount();
    await screen.findByText('筛选结果 21 条');
    request.mockResolvedValue({ ...data(), total: 21, offset: 20, counts: [{ outcome: 'failed', count: 21 }] });
    await userEvent.click(screen.getByText('2', { selector: '.semi-page-item' }));
    await waitFor(() => expect(request).toHaveBeenLastCalledWith(`${root}?limit=20&offset=20`, expect.anything()));
    request.mockResolvedValue(data());
    await userEvent.click(within(screen.getByText('任务类型', { selector: '.admin-filter-label' }).closest('label')!).getByRole('combobox'));
    await userEvent.click(await screen.findByText('理财到期赎回', { selector: '.semi-select-option-text' }));
    await userEvent.click(screen.getByRole('button', { name: '查询' }));
    await waitFor(() => expect(request).toHaveBeenLastCalledWith(`${root}?task_kind=earn&limit=20&offset=0`, expect.anything()));
  });
  it('秒合约显示真实复核证据，尝试字段为空，按权限提供原订单深链但无重新排期', async () => {
    request.mockResolvedValue(data({
      task_kind: 'seconds', outcome: 'manual_review', attempt_count: null, next_attempt_at: null, last_attempt_at: null,
      source_type: 'seconds_contract_order', source_order_id: '7', failure_code: 'price_missing',
      failure_at: 1_789_700_000_000, review_window_start: 1_789_600_000_000, review_window_end: 1_789_700_000_000
    }));
    mount(['governance.financial.read', 'governance.financial.operate', 'seconds.orders.read']);
    await screen.findByText('待人工复核 1 条');
    expect(screen.getByText('price_missing')).toBeInTheDocument();
    expect(screen.getByText('非后台重试任务')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '7' })).toHaveAttribute('href', '/admin/seconds-contract/orders/7');
    expect(screen.queryByRole('button', { name: /重新排期/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '跟进信息 秒合约人工复核 7' })).toBeEnabled();
    expect(request.mock.calls.every(([, options]) => !options?.method)).toBe(true);
  });
  it('无秒合约读取权限时只显示来源标识，不提供订单深链', async () => {
    request.mockResolvedValue(data({
      task_kind: 'seconds', outcome: 'manual_review', attempt_count: null, next_attempt_at: null, last_attempt_at: null,
      source_type: 'seconds_contract_order', source_order_id: '7', failure_code: 'price_missing',
      failure_at: 1_789_700_000_000, review_window_start: 1_789_600_000_000, review_window_end: 1_789_700_000_000
    }));
    mount();
    await screen.findByText('待人工复核 1 条');
    expect(screen.queryByRole('link', { name: '7' })).not.toBeInTheDocument();
    expect(request).toHaveBeenCalledTimes(1);
  });
  it('跟进信息默认无期限；原因必填，409 保留草稿，只 PATCH 元数据', async () => {
    request.mockResolvedValue(data());
    mount(['governance.financial.read', 'governance.financial.operate']);
    await screen.findByText('未设置期限');
    await userEvent.click(screen.getByRole('button', { name: '跟进信息 理财到期赎回 7' }));
    const save = screen.getByRole('button', { name: '保存跟进信息' });
    expect(save).toBeDisabled();
    expect(screen.getByRole('textbox', { name: '负责人（管理员 ID）' })).toHaveValue('');
    expect(screen.getByPlaceholderText('未设置')).toHaveValue('');
    await userEvent.type(screen.getByRole('textbox', { name: '负责人（管理员 ID）' }), '12');
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '分派核查');
    request.mockRejectedValue(new ApiError(409, 'CONFLICT', '跟进版本已变化'));
    await userEvent.click(save);
    await screen.findByText('跟进版本已变化');
    expect(screen.getByRole('textbox', { name: '操作原因' })).toHaveValue('分派核查');
    expect(request).toHaveBeenLastCalledWith(`${root}/earn/7/incident`, {
      method: 'PATCH', body: JSON.stringify({ expected_version: 0, owner_admin_id: 12, due_at: null, reason: '分派核查' })
    });
    expect(request.mock.calls.filter(([, options]) => options?.method === 'PATCH')).toHaveLength(1);
  });
  it('处理期限依据服务端快照判逾期，只读角色不能改负责人', async () => {
    request.mockResolvedValue(data({ incident: {
      version: 1, owner_admin_id: 12, owner_name: 'ops', due_at: 1_789_699_000_000, updated_by: 1, updated_at: 1_789_600_000_000
    } }));
    mount();
    await screen.findByText('已逾期');
    expect(screen.getByText('ops (#12)')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /跟进信息/ })).not.toBeInTheDocument();
  });
  it('保留操作员设置的毫秒期限，保存仅更新元数据并刷新', async () => {
    const incident = {
      version: 3, owner_admin_id: 12, owner_name: 'ops', due_at: 1_789_900_000_123, updated_by: 1, updated_at: 1_789_600_000_000
    };
    request.mockResolvedValue(data({ incident }));
    mount(['governance.financial.read', 'governance.financial.operate']);
    await userEvent.click(await screen.findByRole('button', { name: '跟进信息 理财到期赎回 7' }));
    await userEvent.type(screen.getByRole('textbox', { name: '操作原因' }), '确认跟进期限');
    request.mockImplementation(async (_path, options) => options?.method === 'PATCH'
      ? { ...incident, version: 4 } : data({ incident: { ...incident, version: 4 } }));
    await userEvent.click(screen.getByRole('button', { name: '保存跟进信息' }));
    await screen.findByText('负责人和处理期限已记录，资金与订单状态未变更。');
    expect(request).toHaveBeenCalledWith(`${root}/earn/7/incident`, {
      method: 'PATCH', body: JSON.stringify({ expected_version: 3, owner_admin_id: 12, due_at: 1_789_900_000_123, reason: '确认跟进期限' })
    });
    expect(request.mock.calls.filter(([, options]) => options?.method === 'PATCH')).toHaveLength(1);
  });
  it('原订单深链读取精确 ID，没有结算或资金动作', async () => {
    request.mockResolvedValue({ id: 7, user_id: 8, status: 'manual_review', stake_amount: '1.123456789012345678' });
    const router = createMemoryRouter([{ path: '/admin/seconds-contract/orders/:orderId', element: <FinancialRetrySecondsOrderPage /> }], {
      initialEntries: ['/admin/seconds-contract/orders/7']
    });
    render(<QueryClientProvider client={new QueryClient()}><RouterProvider router={router} /></QueryClientProvider>);
    await screen.findByRole('link', { name: '订单列表' });
    await waitFor(() => expect(request).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/orders/7', expect.anything()));
    expect(screen.queryByRole('button', { name: /结算|退款|判赢|判输/ })).not.toBeInTheDocument();
    expect(request.mock.calls.every(([, options]) => !options?.method)).toBe(true);
  });
});
