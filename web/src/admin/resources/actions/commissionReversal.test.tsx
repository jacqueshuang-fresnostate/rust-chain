import { Toast } from '@douyinfe/semi-ui';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../api/client';
import { authStore } from '../../../auth/authStore';
import { FINANCIAL_COMMAND_STORAGE_KEY } from '../../../shared/idempotency';
import { AdminAccessProvider } from '../../access';
import { resourceConfigs } from '../resourceConfigs';
import { CommissionReversalAction } from './commissionReversal';

vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'), apiRequest: vi.fn()
}));
const helpers = { loadDetail: vi.fn(), openDetail: vi.fn(), reload: vi.fn() };
const record = { id: 31, payout_asset_id: 2, commission_amount: '5.000000000000000000', status: 'settled' };
const access = { admin_id: 7, username: 'operator', role_id: 1, role_name: 'operations', is_super_admin: false };

function view(permissions = ['agents.commissions.settle'], status = 'settled') {
  return render(<AdminAccessProvider access={{ ...access, permissions }}>
    <CommissionReversalAction helpers={helpers} record={{ ...record, status }} />
  </AdminAccessProvider>);
}
async function confirm(label = '冲正') {
  await userEvent.click(screen.getByRole('button', { name: label }));
  expect(await screen.findByRole('button', { name: '确认' })).toBeDisabled();
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: '  source revoked  ' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
}
function bodies() {
  return vi.mocked(apiRequest).mock.calls.map(([, init]) => JSON.parse(init!.body as string) as Record<string, unknown>);
}
function success() {
  vi.mocked(apiRequest).mockImplementation(async (_, init) => ({
    ...JSON.parse(init!.body as string), commission_id: 31, admin_id: 7, asset_id: 2,
    amount: record.commission_amount, created_at: 1789700000000
  }));
}
beforeEach(() => {
  vi.clearAllMocks();
  sessionStorage.clear();
  authStore.setSession({ accessToken: 'test', refreshToken: 'refresh', scope: 'admin', subject: 'admin:7' });
  vi.spyOn(Toast, 'success').mockImplementation(() => 'success');
  success();
});
afterEach(() => vi.restoreAllMocks());

it('requires exact settlement permission and refuses pending commissions', () => {
  const readonly = view(['agents.commissions.read']);
  expect(screen.queryByRole('button', { name: '冲正' })).not.toBeInTheDocument();
  readonly.unmount();
  view(['agents.commissions.settle'], 'pending');
  expect(screen.getByRole('button', { name: '冲正' })).toBeDisabled();
  expect(apiRequest).not.toHaveBeenCalled();
});

it('requires a reason and submits only the stable key and reason, never an amount or actor', async () => {
  view();
  await confirm();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(apiRequest).toHaveBeenCalledWith('/admin/api/v1/agent-commissions/31/reversal', {
    method: 'POST', body: expect.any(String)
  });
  expect(bodies()).toEqual([{ reason: 'source revoked', idempotency_key: expect.stringMatching(/^commission-reversal-/) }]);
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toBeNull();
});

it('renders the real resource row wrapper for a settle-only operator and reaches the exact POST endpoint', async () => {
  render(<AdminAccessProvider access={{ ...access, permissions: ['agents.commissions.settle'] }}>
    {resourceConfigs.agentCommissions.rowActions?.(record, helpers)}
  </AdminAccessProvider>);
  await screen.findByRole('button', { name: '冲正' });
  expect(screen.queryByRole('button', { name: '结算' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '拒绝' })).not.toBeInTheDocument();
  await confirm();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(apiRequest).toHaveBeenCalledWith('/admin/api/v1/agent-commissions/31/reversal', {
    method: 'POST', body: expect.any(String)
  });
});

it('keeps the same key after response loss and remount, including a now-reversed row', async () => {
  vi.mocked(apiRequest).mockRejectedValue(new Error('response lost'));
  const first = view();
  await confirm();
  await screen.findByRole('alert');
  const original = bodies()[0];
  expect(helpers.reload).not.toHaveBeenCalled();
  expect(screen.getByLabelText('操作原因')).toHaveValue('  source revoked  ');
  first.unmount();
  success();
  view(['agents.commissions.settle'], 'reversed');
  await confirm('核对冲正');
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(bodies()[1]).toEqual(original);
});

it('does not claim success or discard identity for a malformed success receipt', async () => {
  vi.mocked(apiRequest).mockResolvedValue({ commission_id: 31 });
  view();
  await confirm();
  expect(await screen.findByRole('alert')).toHaveTextContent('冲正结果尚未确认');
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toContain(String(bodies()[0].idempotency_key));
  expect(helpers.reload).not.toHaveBeenCalled();
});

it('refuses a new command on an already-reversed row and fails closed without durable storage', async () => {
  const first = view(['agents.commissions.settle'], 'reversed');
  await confirm('核对冲正');
  expect(await screen.findByRole('alert')).toHaveTextContent('仅可冲正已支付佣金');
  expect(apiRequest).not.toHaveBeenCalled();
  first.unmount();
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('storage unavailable'); });
  view();
  await confirm();
  await screen.findByRole('alert');
  expect(apiRequest).not.toHaveBeenCalled();
});
