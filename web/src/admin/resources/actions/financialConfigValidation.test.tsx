import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../../api/adminResources';
import { apiRequest } from '../../../api/client';
import { ConvertPairRowActions, convertPairValidationError, type ConvertPairValues } from './convert';
import { UserRowActions } from './users';
import { authStore } from '../../../auth/authStore';
import { financialCommandIntents, financialCommandScopeFromSession } from '../../../shared/idempotency';

vi.mock('../../../api/adminResources', () => ({ listAdminResource: vi.fn() }));
vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'),
  apiRequest: vi.fn()
}));

const pair = {
  id: 7, from_asset_id: 12, to_asset_id: 13, pricing_mode: 'fixed',
  spread_rate: '0.01', fee_rate: '0', min_amount: '1', max_amount: '100',
  target_min_amount: '0', target_max_amount: null, enabled: true
};
const helpers = { reload: vi.fn(), openDetail: vi.fn(), loadDetail: vi.fn(async () => {}) };
const validValues: ConvertPairValues = {
  fromAssetId: '12', toAssetId: '13', pricingMode: 'market', spreadRate: '0.01', feeRate: '0',
  minAmount: '0', maxAmount: '', targetMinAmount: '1', targetMaxAmount: '100', enabled: 'true'
};

async function openRecharge() {
  authStore.setSession({ accessToken: 'test-admin-access', refreshToken: 'test-admin-refresh', scope: 'admin', subject: 'admin:7' });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><UserRowActions helpers={helpers} record={{ id: 42, status: 'active' }} /></QueryClientProvider>);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '充值' }));
  await waitFor(() => expect(listAdminResource).toHaveBeenCalled());
  const select = screen.getByText('充值资产', { selector: 'label' }).querySelector('.semi-select')!;
  await user.click(select);
  const option = await screen.findByText('USD（ID: 12）', { selector: '.semi-select-option-text' });
  fireEvent.mouseDown(option);
  fireEvent.click(option);
  await screen.findByText('该资产最多支持 2 位小数，超出精度不会自动舍入。');
  return user;
}

describe('financial configuration validation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sessionStorage.clear();
    vi.mocked(apiRequest).mockResolvedValue({});
    vi.mocked(listAdminResource).mockResolvedValue({ rows: [
      { id: 12, symbol: 'USD', precision_scale: 2 },
      { id: 13, symbol: 'BTC', precision_scale: 18 }
    ], raw: {} });
  });

  it.each([
    { pricingMode: 'markte' }, { spreadRate: '-0.01' }, { spreadRate: '1' }, { feeRate: '1' },
    { feeRate: '1e-9' }, { minAmount: '-1' }, { targetMinAmount: '-1' },
    { minAmount: '2', maxAmount: '1' }, { targetMaxAmount: '0.5' },
    { fromAssetId: '013' }, { fromAssetId: 'not-an-id' }
  ])('rejects invalid convert configuration %j', (patch) => {
    expect(convertPairValidationError({ ...validValues, ...patch })).not.toBeNull();
  });

  it('preserves valid boundaries and equivalent trailing zeros', () => {
    expect(convertPairValidationError({ ...validValues, spreadRate: '0.9999999900', feeRate: '1e-8', maxAmount: '0.000' })).toBeNull();
  });

  it('rejects a new excess-precision recharge before a request and accepts trailing zeros', async () => {
    const user = await openRecharge();
    fireEvent.change(screen.getByLabelText('充值金额'), { target: { value: '1.231' } });
    await user.click(screen.getByRole('button', { name: '提交充值' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '人工充值核对' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await screen.findByText('充值金额最多支持 2 位小数，不会自动舍入');
    expect(apiRequest).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '取消' }));
    fireEvent.change(screen.getByLabelText('充值金额'), { target: { value: '1.2300' } });
    await user.click(screen.getByRole('button', { name: '提交充值' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '人工充值核对' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequest).toHaveBeenCalledTimes(1));
    const body = JSON.parse(vi.mocked(apiRequest).mock.calls[0][1]!.body as string);
    expect(body).toMatchObject({ asset_id: 12, amount: '1.23', reason: '人工充值核对' });
    expect(body.idempotency_key).toEqual(expect.any(String));
  });

  it('allows only the exact pending intent to replay after asset precision tightens', async () => {
    const user = await openRecharge();
    const scope = financialCommandScopeFromSession(authStore.getSession('admin')!, 'admin-user-recharge', 42, 12);
    const values = { user_id: 42, asset_id: 12, amount: '1.231', reason: '原请求核对' };
    const lease = financialCommandIntents.acquire(scope, values);
    financialCommandIntents.markUncertain(lease);
    fireEvent.change(screen.getByLabelText('充值金额'), { target: { value: '1.2310' } });
    await user.click(screen.getByRole('button', { name: '提交充值' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '不同原因' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await screen.findByText('充值金额最多支持 2 位小数，不会自动舍入');
    expect(apiRequest).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: values.reason } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequest).toHaveBeenCalledWith('/admin/api/v1/users/42/recharge', {
      method: 'POST', body: JSON.stringify({ asset_id: 12, amount: '1.231', reason: values.reason, idempotency_key: lease.key })
    }));
    expect(financialCommandIntents.hasPending(scope, values)).toBe(false);
  });

  it('blocks an unusable spread and reversed amount range in the edit form', async () => {
    const user = userEvent.setup();
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><ConvertPairRowActions helpers={helpers} record={pair} /></QueryClientProvider>);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const submit = await screen.findByRole('button', { name: '提交修改' });
    expect(submit).toBeEnabled();
    fireEvent.change(screen.getByLabelText('价差率'), { target: { value: '1' } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByLabelText('价差率'), { target: { value: '0.999999999' } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByLabelText('价差率'), { target: { value: '0.99999999' } });
    expect(submit).toBeEnabled();
    fireEvent.change(screen.getByLabelText('源资产最大金额'), { target: { value: '0.5' } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByLabelText('源资产最大金额'), { target: { value: '' } });
    expect(submit).toBeEnabled();
    await user.click(submit);
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '修正有效配置' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequest).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/7', {
      method: 'PATCH', body: JSON.stringify({
        from_asset_id: 12, to_asset_id: 13, pricing_mode: 'fixed', spread_rate: '0.99999999',
        fee_rate: '0', min_amount: '1', max_amount: null, target_min_amount: '0',
        target_max_amount: null, enabled: true, reason: '修正有效配置'
      })
    }));
  });
});
