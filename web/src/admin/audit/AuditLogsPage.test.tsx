import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { type ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import type { AdminAuditLog, AdminAuditLogsResponse } from './auditApi';
import { AuditLogsPage } from './AuditLogsPage';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return { ...actual, apiRequest: vi.fn() };
});

const apiRequestMock = vi.mocked(apiRequest);

function auditLog(overrides: Partial<AdminAuditLog> = {}): AdminAuditLog {
  return {
    action: 'asset.config.update',
    admin_id: 7,
    after_json: {
      enabled: true,
      fee_rate: '0.002',
      nested: { password: 'new-password', api_key: 'new-api-key', accessToken: 'new-token' }
    },
    before_json: {
      enabled: false,
      fee_rate: '0.001',
      nested: { password: 'old-password', api_key: 'old-api-key', accessToken: 'old-token' }
    },
    created_at: 1_735_732_800_000,
    id: 99,
    ip: '203.0.113.9',
    reason: '调整资产配置，token=reason-secret',
    request_id: 'req-audit-99',
    target_id: '9',
    target_type: 'asset',
    ...overrides
  };
}

function response(logs: AdminAuditLog[] = [auditLog()]): AdminAuditLogsResponse {
  return { logs, total: logs.length };
}

function renderPage(): ReturnType<typeof render> {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>
  );
  return render(<AuditLogsPage />, { wrapper });
}

describe('AuditLogsPage', () => {
  beforeEach(() => {
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue(response());
  });

  it('renders Chinese actions, object links, field-level differences, actor context, and recursive masks', async () => {
    renderPage();

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/audit-logs?limit=20&offset=0');
    });
    expect(await screen.findByText('更新资产')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看资产 #9' })).toHaveAttribute('href', '/admin/assets');
    expect(screen.getByText('启用状态')).toBeInTheDocument();
    expect(screen.getByText('手续费率')).toBeInTheDocument();
    expect(screen.getAllByText('旧值').length).toBeGreaterThan(0);
    expect(screen.getAllByText('新值').length).toBeGreaterThan(0);
    expect(screen.getByText('调整资产配置，token=***')).toBeInTheDocument();
    expect(screen.getByText('管理员 #7')).toBeInTheDocument();
    expect(screen.getByText('203.0.113.9')).toBeInTheDocument();
    expect(screen.getByText('req-audit-99')).toBeInTheDocument();
    expect(screen.getAllByText('敏感内容已遮罩').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '导出当前结果' })).toBeEnabled();
    const pageText = document.body.textContent ?? '';
    for (const secret of [
      'old-password',
      'new-password',
      'old-api-key',
      'new-api-key',
      'old-token',
      'new-token',
      'reason-secret'
    ]) {
      expect(pageText).not.toContain(secret);
    }
  });

  it('masks credentials embedded in ordinary snapshot and trace text before rendering', async () => {
    apiRequestMock.mockResolvedValueOnce(response([
      auditLog({
        before_json: { diagnostic: '{"token":"old-inline-token"}' },
        after_json: { diagnostic: 'password=new-inline-password' },
        ip: 'token=ip-secret',
        request_id: 'Bearer request-secret'
      })
    ]));

    renderPage();

    expect((await screen.findAllByText('token=***')).length).toBeGreaterThan(0);
    expect(screen.getByText('password=***')).toBeInTheDocument();
    expect(screen.getByText('Bearer ***')).toBeInTheDocument();
    const pageText = document.body.textContent ?? '';
    for (const secret of ['old-inline-token', 'new-inline-password', 'ip-secret', 'request-secret']) {
      expect(pageText).not.toContain(secret);
    }
  });

  it('queries the backend DTO with exact inclusive Unix-millisecond time range and supported filters', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('更新资产');

    await user.type(screen.getByLabelText('管理员 ID'), ' 42 ');
    await user.type(screen.getByLabelText('动作'), ' asset.config.update ');
    await user.type(screen.getByLabelText('对象类型'), ' asset ');
    await user.type(screen.getByLabelText('对象 ID'), ' 9 ');
    fireEvent.change(screen.getByLabelText('起始时间'), { target: { value: '2026-08-18T08:30:15' } });
    fireEvent.change(screen.getByLabelText('结束时间'), { target: { value: '2026-08-18T09:45:30' } });
    await user.click(screen.getByRole('button', { name: '查询审计日志' }));

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(2));
    const requestPath = String(apiRequestMock.mock.calls.at(-1)?.[0]);
    const url = new URL(requestPath, 'http://admin.local');
    expect(url.pathname).toBe('/admin/api/v1/audit-logs');
    expect([...url.searchParams.keys()]).toEqual([
      'admin_id',
      'action',
      'target_type',
      'target_id',
      'created_from',
      'created_to',
      'limit',
      'offset'
    ]);
    expect(url.searchParams.get('admin_id')).toBe('42');
    expect(url.searchParams.get('action')).toBe('asset.config.update');
    expect(url.searchParams.get('target_type')).toBe('asset');
    expect(url.searchParams.get('target_id')).toBe('9');
    expect(url.searchParams.get('created_from')).toBe(String(new Date('2026-08-18T08:30:15').getTime()));
    expect(url.searchParams.get('created_to')).toBe(String(new Date('2026-08-18T09:45:30').getTime()));
    expect(url.searchParams.get('limit')).toBe('20');
    expect(url.searchParams.get('offset')).toBe('0');
    expect(url.searchParams.has('from')).toBe(false);
    expect(url.searchParams.has('to')).toBe(false);
  });

  it('rejects an inverted time range before sending another request', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('更新资产');
    fireEvent.change(screen.getByLabelText('起始时间'), { target: { value: '2026-08-18T10:00:00' } });
    fireEvent.change(screen.getByLabelText('结束时间'), { target: { value: '2026-08-18T09:00:00' } });

    await user.click(screen.getByRole('button', { name: '查询审计日志' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('起始时间不得晚于结束时间');
    expect(apiRequestMock).toHaveBeenCalledTimes(1);
  });

  it('shows explicit loading and empty states', async () => {
    let resolveRequest: ((value: AdminAuditLogsResponse) => void) | undefined;
    apiRequestMock.mockReset();
    apiRequestMock.mockReturnValueOnce(new Promise((resolve) => {
      resolveRequest = resolve;
    }));

    renderPage();
    expect(screen.getByText('正在加载审计日志…')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '导出当前结果' })).toBeDisabled();
    await act(async () => resolveRequest?.(response([])));
    expect(await screen.findByText('没有符合条件的审计日志')).toBeInTheDocument();
    expect(screen.getByText(/可放宽时间范围/)).toBeInTheDocument();
  });

  it('shows a safe error, retries, and then renders the empty state', async () => {
    const user = userEvent.setup();
    apiRequestMock
      .mockRejectedValueOnce(new Error('network down\nprivate backend stack'))
      .mockResolvedValueOnce(response([]));

    renderPage();

    expect(await screen.findByText('审计日志加载失败')).toBeInTheDocument();
    expect(screen.getByText('network down')).toBeInTheDocument();
    expect(screen.queryByText(/private backend stack/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '重新加载' }));
    expect(await screen.findByText('没有符合条件的审计日志')).toBeInTheDocument();
    expect(apiRequestMock).toHaveBeenCalledTimes(2);
  });

  it('explains logs with no recorded or computed differences', async () => {
    apiRequestMock.mockResolvedValueOnce(response([
      auditLog({ id: 100, before_json: null, after_json: null }),
      auditLog({ id: 101, before_json: { enabled: true }, after_json: { enabled: true } })
    ]));

    renderPage();

    expect(await screen.findByText('未记录前后快照，暂无字段差异。')).toBeInTheDocument();
    expect(screen.getByText('前后快照一致，未发现字段变化。')).toBeInTheDocument();
  });
});
