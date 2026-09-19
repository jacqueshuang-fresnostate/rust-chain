import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest } from '../../api/client';
import { WithdrawalPolicyPage } from './WithdrawalPolicyPage';
import { emptyWithdrawalPolicy } from './withdrawalPolicy';

vi.mock('../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../api/client')>('../../api/client'),
  apiRequest: vi.fn()
}));

const apiMock = vi.mocked(apiRequest);
const endpoint = '/admin/api/v1/wallet/withdrawal-policies/7';

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } } });
  const router = createMemoryRouter([{ path: '/policy', element: <WithdrawalPolicyPage /> }], { initialEntries: ['/policy?asset_id=7'] });
  render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
}

describe('WithdrawalPolicyPage', () => {
  beforeEach(() => {
    apiMock.mockReset();
    apiMock.mockImplementation(async (path, init) => {
      if (path === endpoint) {
        if (init?.method === 'PATCH') return { asset_id: 7, revision: 5, policy: JSON.parse(String(init.body)).policy };
        return { asset_id: 7, revision: 4, policy: emptyWithdrawalPolicy };
      }
      return { assets: [], total: 0 };
    });
  });

  it('saves explicit inactive configuration with revision, reason and decimal text', async () => {
    renderPage();
    expect(await screen.findByRole('button', { name: '保存提现策略' })).toBeDisabled();
    expect(screen.getByRole('switch', { name: '启用策略' })).not.toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: '添加额度规则' }));
    fireEvent.change(screen.getByLabelText('规则1窗口秒数'), { target: { value: '3600' } });
    fireEvent.change(screen.getByLabelText('规则1累计上限'), { target: { value: '0.000000000000000001' } });
    fireEvent.click(screen.getByRole('button', { name: '保存提现策略' }));
    expect(await screen.findByRole('button', { name: '确认保存' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: '  approved fixture  ' } });
    fireEvent.click(screen.getByRole('button', { name: '确认保存' }));
    await waitFor(() => expect(apiMock).toHaveBeenCalledWith(endpoint, expect.objectContaining({ method: 'PATCH' })));
    const body = JSON.parse(String(apiMock.mock.calls.find(([, init]) => init?.method === 'PATCH')?.[1]?.body));
    expect(body.expected_revision).toBe(4);
    expect(body.reason).toBe('approved fixture');
    expect(body.policy.enabled).toBe(false);
    expect(body.policy.allowances[0].max_amount).toBe('0.000000000000000001');
    expect(await screen.findByText('提现策略已保存')).toBeInTheDocument();
  });

  it('refuses malformed policy instead of displaying disabled success', async () => {
    apiMock.mockResolvedValue({ asset_id: 7, revision: 4, policy: { enabled: false } });
    renderPage();
    expect(await screen.findByText('配置加载失败')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '保存提现策略' })).not.toBeInTheDocument();
  });

  it('does not round unsafe or fractional integer inputs into valid restrictions', async () => {
    renderPage();
    await screen.findByRole('button', { name: '保存提现策略' });
    fireEvent.click(screen.getByRole('button', { name: '添加额度规则' }));
    fireEvent.change(screen.getByLabelText('规则1窗口秒数'), { target: { value: '3600' } });
    fireEvent.change(screen.getByLabelText('规则1累计上限'), { target: { value: '1' } });
    for (const value of ['9007199254740993', '1.000000000000000001', '1e3', 'NaN', 'Infinity']) {
      fireEvent.change(screen.getByLabelText('规则1用户'), { target: { value } });
      expect(screen.getByLabelText('规则1用户')).toHaveValue(value);
      expect(screen.getByRole('button', { name: '保存提现策略' })).toBeDisabled();
    }
    expect(apiMock.mock.calls.some(([, init]) => init?.method === 'PATCH')).toBe(false);
  });
});
