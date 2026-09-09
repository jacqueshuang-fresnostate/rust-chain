import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ApiError, ContractError } from '../../api/client';
import { AdminAccessProvider } from '../access';
import {
  NewCoinReconciliationPage,
  parseNewCoinReconciliation,
  type NewCoinReconciliation
} from './NewCoinReconciliationPage';

vi.mock('../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../api/client')>('../../api/client'),
  apiRequest: vi.fn()
}));

const request = vi.mocked(apiRequest);

if (!Range.prototype.getBoundingClientRect) {
  Range.prototype.getBoundingClientRect = () => new DOMRect(0, 0, 0, 0);
}

function reconciliation(overrides: Partial<NewCoinReconciliation> = {}): NewCoinReconciliation {
  return {
    project_id: 7,
    symbol: 'HIP',
    lifecycle_status: 'distribution',
    total_supply: '100.000000000000000001',
    reserved_supply: '10',
    allocated_supply: '20',
    remaining_supply: '70.000000000000000001',
    supply_delta: '0',
    subscription_count: 2,
    pending_manual_count: 0,
    requested_quantity: '30',
    subscription_allocated_quantity: '20',
    distribution_quantity: '20',
    linked_distribution_quantity: '20',
    unlinked_distribution_quantity: '0',
    distribution_ledger_quantity: '20',
    invalid_subscription_link_count: 0,
    subscription_distribution_delta: '0',
    distribution_ledger_delta: '0',
    manual_quote_amount: '50',
    manual_frozen_quote_amount: '0',
    manual_settled_quote_amount: '40',
    manual_refunded_quote_amount: '10',
    manual_quote_delta: '0',
    anomaly_count: 0,
    anomalies: [],
    status: 'balanced',
    checked_at: 1_788_883_200_123,
    ...overrides
  };
}

function mount(permissions = ['*'], entry = '/admin/new-coins/reconciliation/7') {
  const router = createMemoryRouter(
    [
      { path: '/admin/new-coins/reconciliation/:projectId', element: <NewCoinReconciliationPage /> },
      { path: '/admin/new-coins/projects/:projectId', element: <div>项目中心目标</div> }
    ],
    { initialEntries: [entry] }
  );
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <AdminAccessProvider
        access={{
          admin_id: 1,
          is_super_admin: permissions.includes('*'),
          permissions,
          role_id: 2,
          role_name: '审计员',
          username: 'auditor'
        }}
      >
        <RouterProvider router={router} />
      </AdminAccessProvider>
    </QueryClientProvider>
  );
  return router;
}

beforeEach(() => {
  request.mockReset();
});

describe('new coin reconciliation response contract', () => {
  it('保留高精度 Decimal 文本与毫秒时间', () => {
    const parsed = parseNewCoinReconciliation(reconciliation(), '7');
    expect(parsed.total_supply).toBe('100.000000000000000001');
    expect(parsed.checked_at).toBe(1_788_883_200_123);
  });

  it('对项目身份、Decimal、计数和异常数组关系失败关闭', () => {
    expect(() => parseNewCoinReconciliation(reconciliation(), '8')).toThrow(ContractError);
    expect(() => parseNewCoinReconciliation({ ...reconciliation(), total_supply: 100 }, '7')).toThrow(ContractError);
    expect(() => parseNewCoinReconciliation({ ...reconciliation(), pending_manual_count: -1 }, '7')).toThrow(ContractError);
    expect(() => parseNewCoinReconciliation({ ...reconciliation(), anomaly_count: 1 }, '7')).toThrow(ContractError);
    expect(() => parseNewCoinReconciliation({ ...reconciliation(), status: 'attention' }, '7')).toThrow(ContractError);
    expect(() => parseNewCoinReconciliation({ ...reconciliation(), checked_at: '1788883200123' }, '7')).toThrow(ContractError);
  });
});

describe('new coin reconciliation page', () => {
  it('显示中文守恒结果并刷新同一个只读端点', async () => {
    request.mockResolvedValue(reconciliation());
    mount();

    await screen.findByText('HIP · 派发与退款对账');
    expect(screen.getByText('对账平衡')).toBeInTheDocument();
    expect(screen.getByText('未发现需要人工核查的差异。')).toBeInTheDocument();
    expect(screen.getByText('人工申购资金守恒')).toBeInTheDocument();
    expect(request).toHaveBeenCalledWith('/admin/api/v1/new-coins/7/reconciliation', expect.objectContaining({ signal: expect.any(AbortSignal) }));

    await userEvent.click(screen.getByRole('button', { name: '刷新对账' }));
    await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
  });

  it('按资源读权限隐藏项目、申购和审计深链', async () => {
    request.mockResolvedValue(reconciliation());
    mount(['new_coin.distributions.read']);

    await screen.findByText('HIP · 派发与退款对账');
    expect(screen.queryByRole('link', { name: '返回项目中心' })).not.toBeInTheDocument();
    expect(screen.queryByRole('link', { name: '查看申购与配售记录' })).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看派发与退款记录' })).toHaveAttribute(
      'href',
      '/admin/new-coins/distributions?project_id=7'
    );
    expect(screen.queryByRole('link', { name: '查看项目审计日志' })).not.toBeInTheDocument();
  });

  it('直接展示后端对账异常与需核查状态', async () => {
    request.mockResolvedValue(reconciliation({
      anomalies: ['派发数量与钱包入账流水不一致'],
      anomaly_count: 1,
      distribution_ledger_delta: '1',
      status: 'attention'
    }));
    mount();

    await screen.findByText('需要核查');
    expect(screen.getByText('派发数量与钱包入账流水不一致')).toBeInTheDocument();
    expect(screen.getByText('发现 1 项差异，请按下方异常项核对原始申购、派发与钱包流水。')).toBeInTheDocument();
  });

  it('对无效项目号提供明确中文状态且不发请求', async () => {
    mount(['*'], '/admin/new-coins/reconciliation/not-a-project');
    expect(await screen.findByText('项目编号无效')).toBeInTheDocument();
    expect(request).not.toHaveBeenCalled();
  });

  it('对加载失败显示中文诊断和重试入口', async () => {
    request.mockRejectedValue(new ApiError(500, 'INTERNAL_ERROR', 'aggregation failed'));
    mount();

    expect(await screen.findByRole('alert')).toHaveTextContent('服务处理异常');
    expect(screen.getByRole('button', { name: '重新加载' })).toBeInTheDocument();
  });
});
