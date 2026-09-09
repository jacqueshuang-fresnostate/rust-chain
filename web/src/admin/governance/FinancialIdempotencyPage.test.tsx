import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ApiError, ContractError } from '../../api/client';
import {
  FinancialIdempotencyPage,
  parseFinancialIdempotencyAudit,
  type FinancialIdempotencyAudit
} from './FinancialIdempotencyPage';

vi.mock('../../api/client', async () => ({
  ...(await vi.importActual<typeof import('../../api/client')>('../../api/client')),
  apiRequest: vi.fn()
}));

const request = vi.mocked(apiRequest);

if (!Range.prototype.getBoundingClientRect) {
  Range.prototype.getBoundingClientRect = () => new DOMRect(0, 0, 0, 0);
}

function audit(overrides: Partial<FinancialIdempotencyAudit> = {}): FinancialIdempotencyAudit {
  return {
    status: 'balanced',
    duplicate_idempotency_groups: 0,
    missing_idempotency_keys: 0,
    orphan_ledger_entries: 0,
    expired_seconds_orders: 0,
    expired_prediction_orders: 0,
    pending_loan_orders: 2,
    pending_new_coin_subscriptions: 3,
    anomaly_count: 0,
    anomalies: [],
    checked_at: 1_788_883_200_123,
    ...overrides
  };
}

function mount() {
  const router = createMemoryRouter(
    [{ path: '/admin/governance/financial-idempotency', element: <FinancialIdempotencyPage /> }],
    { initialEntries: ['/admin/governance/financial-idempotency'] }
  );
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}

beforeEach(() => request.mockReset());

describe('financial idempotency audit response contract', () => {
  it('保留非负安全整数和毫秒时间', () => {
    const parsed = parseFinancialIdempotencyAudit(audit());
    expect(parsed.pending_new_coin_subscriptions).toBe(3);
    expect(parsed.checked_at).toBe(1_788_883_200_123);
  });

  it('对状态、计数、时间和异常数组关系失败关闭', () => {
    expect(() => parseFinancialIdempotencyAudit({ ...audit(), status: 'unknown' })).toThrow(
      ContractError
    );
    expect(() =>
      parseFinancialIdempotencyAudit({ ...audit(), orphan_ledger_entries: -1 })
    ).toThrow(ContractError);
    expect(() => parseFinancialIdempotencyAudit({ ...audit(), anomaly_count: 1 })).toThrow(
      ContractError
    );
    expect(() => parseFinancialIdempotencyAudit({ ...audit(), checked_at: 'now' })).toThrow(
      ContractError
    );
  });
});

describe('financial idempotency audit page', () => {
  it('显示中文审计结果、正常待办，并刷新同一只读端点', async () => {
    request.mockResolvedValue(audit());
    mount();

    await screen.findByText('对账平衡');
    expect(screen.getByText('未发现需要人工核查的资金完整性异常。')).toBeInTheDocument();
    expect(screen.getByText(/^待审核贷款订单/)).toBeInTheDocument();
    expect(screen.getByText(/^待派发或退款新币申购/)).toBeInTheDocument();
    expect(request).toHaveBeenCalledWith(
      '/admin/api/v1/governance/financial-idempotency',
      expect.objectContaining({ signal: expect.any(AbortSignal) })
    );

    const refresh = screen.getByRole('button', { name: '刷新审计' });
    await userEvent.click(refresh);
    await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(refresh).not.toHaveClass('semi-button-loading'));
  });

  it('直接展示后端异常，不把正常业务待办计入异常', async () => {
    request.mockResolvedValue(
      audit({
        status: 'attention',
        orphan_ledger_entries: 2,
        anomaly_count: 1,
        anomalies: ['存在无法关联业务对象的钱包流水（2 条）']
      })
    );
    mount();

    await screen.findByText('需要核查');
    expect(screen.getByText('存在无法关联业务对象的钱包流水（2 条）')).toBeInTheDocument();
    expect(screen.getByText('异常项 1 条')).toBeInTheDocument();
  });

  it('加载失败时提供中文诊断与重试入口', async () => {
    let rejectRequest: (reason?: unknown) => void = () => undefined;
    request.mockImplementationOnce(
      () => new Promise((_, reject) => {
        rejectRequest = reject;
      })
    );
    mount();
    await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
    await act(async () => {
      rejectRequest(new ApiError(500, 'INTERNAL_ERROR', 'query failed'));
      await Promise.resolve();
    });

    expect(await screen.findByRole('alert')).toHaveTextContent('服务处理异常');
    expect(screen.getByRole('button', { name: '重新加载' })).toBeInTheDocument();
  });
});
