import { Toast } from '@douyinfe/semi-ui';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../api/client';
import { authStore } from '../../../auth/authStore';
import { FINANCIAL_COMMAND_STORAGE_KEY } from '../../../shared/idempotency';
import { AdminAccessProvider } from '../../access';
import { resourceConfigs } from '../resourceConfigs';
import { SecondsPrincipalRefundAction, SecondsRefundPolicyAction, parseSecondsRefundContext, parseSecondsRefundPolicy } from './secondsRefund';

vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'), apiRequest: vi.fn()
}));
const helpers = { loadDetail: vi.fn(), openDetail: vi.fn(), reload: vi.fn() };
const access = { admin_id: 7, username: 'operator', role_id: 1, role_name: 'ops', is_super_admin: false };
const record = { id: 31, stake_asset: 2, stake_asset_symbol: 'USDT', stake_amount: '10.000000000000000000', status: 'manual_review' };
const context = { order_id: 31, policy_version: 3, wait_seconds: 100, eligible_at: 1000, checked_at: 2000, receipt: null };
const policy = { product_id: 4, version: 0, enabled: false, wait_seconds: null };
const permissions = ['seconds.orders.read', 'seconds.orders.settle', 'seconds.products.read', 'seconds.products.write'];
function view(child: React.ReactNode, granted = permissions) {
  return render(<AdminAccessProvider access={{ ...access, permissions: granted }}>{child}</AdminAccessProvider>);
}
function mockRefund(response = context) {
  vi.mocked(apiRequest).mockImplementation(async (_, init) => {
    if (init?.method !== 'POST') return response;
    return { ...JSON.parse(init.body as string), order_id: 31, asset_id: 2, admin_id: 7,
      amount: record.stake_amount, policy_version: 3, created_at: 2100 };
  });
}
async function refund() {
  await userEvent.click(screen.getByRole('button', { name: '本金退还' }));
  await userEvent.click(await screen.findByRole('button', { name: '确认退还本金' }));
  expect(screen.getByRole('button', { name: '确认' })).toBeDisabled();
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: '  no evidence  ' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
}
beforeEach(() => {
  vi.clearAllMocks();
  sessionStorage.clear();
  authStore.setSession({ accessToken: 'test', refreshToken: 'refresh', scope: 'admin', subject: 'admin:7' });
  vi.spyOn(Toast, 'success').mockImplementation(() => 'ok');
});
afterEach(() => vi.restoreAllMocks());

it('loads disabled/null without choosing an interval and requires explicit reason and version to save', async () => {
  vi.mocked(apiRequest).mockImplementation(async (_, init) => init?.method === 'PATCH' ? {
    ...JSON.parse(init.body as string), product_id: 4, version: 1
  } : policy);
  view(<SecondsRefundPolicyAction productId="4" helpers={helpers} />);
  await userEvent.click(screen.getByRole('button', { name: '本金退款策略' }));
  expect(await screen.findByLabelText('首次人工审核后等待秒数')).toHaveValue(null);
  await userEvent.click(screen.getByRole('switch'));
  expect(screen.getByRole('button', { name: '保存退款策略' })).toBeDisabled();
  fireEvent.change(screen.getByLabelText('首次人工审核后等待秒数'), { target: { value: '1800' } });
  await userEvent.click(screen.getByRole('button', { name: '保存退款策略' }));
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: 'explicit future orders' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(apiRequest).toHaveBeenLastCalledWith('/admin/api/v1/seconds-contracts/products/4/refund-policy', {
    method: 'PATCH', body: JSON.stringify({ expected_version: 0, enabled: true, wait_seconds: 1800, reason: 'explicit future orders' })
  });
});

it('retains the policy version, draft and reason on a configuration conflict', async () => {
  vi.mocked(apiRequest).mockImplementation(async (_, init) => {
    if (init?.method === 'PATCH') throw new Error('version conflict');
    return policy;
  });
  view(<SecondsRefundPolicyAction productId="4" helpers={helpers} />);
  await userEvent.click(screen.getByRole('button', { name: '本金退款策略' }));
  await userEvent.click(await screen.findByRole('button', { name: '保存退款策略' }));
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: 'keep disabled' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('version conflict');
  expect(screen.getByLabelText('操作原因')).toHaveValue('keep disabled');
  expect(helpers.reload).not.toHaveBeenCalled();
});

it('uses exact refund POST permission through the real resource row wrapper and never sends an amount', async () => {
  mockRefund();
  view(<>{resourceConfigs.secondsOrders.rowActions?.(record, helpers)}</>);
  await screen.findByRole('button', { name: '本金退还' });
  await refund();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  const call = vi.mocked(apiRequest).mock.calls.find(([, init]) => init?.method === 'POST')!;
  expect(call[0]).toBe('/admin/api/v1/seconds-contracts/orders/31/principal-refund');
  expect(JSON.parse(call[1]!.body as string)).toEqual({ reason: 'no evidence', idempotency_key: expect.stringMatching(/^seconds-refund-/) });
});

it('hides refund from readonly and write-only operators', () => {
  view(<SecondsPrincipalRefundAction helpers={helpers} record={record} />, ['seconds.orders.read', 'seconds.orders.write']);
  expect(screen.queryByRole('button', { name: '本金退还' })).not.toBeInTheDocument();
});

it('refuses legacy orders and does not reinterpret current product configuration as a snapshot', async () => {
  mockRefund({ ...context, policy_version: null, wait_seconds: null, eligible_at: null } as unknown as typeof context);
  view(<SecondsPrincipalRefundAction helpers={helpers} record={record} />);
  await userEvent.click(screen.getByRole('button', { name: '本金退还' }));
  expect(await screen.findByText('开仓时未保存退款策略，本单不可退款。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '确认退还本金' })).toBeDisabled();
});

it('uses only the original order snapshot after product closure and forbids a fresh command for a refunded order', async () => {
  mockRefund();
  const first = view(<SecondsPrincipalRefundAction helpers={helpers} record={{ ...record, product_status: 'disabled' }} />);
  await refund();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(vi.mocked(apiRequest).mock.calls.every(([path]) => path === '/admin/api/v1/seconds-contracts/orders/31/principal-refund')).toBe(true);
  first.unmount();
  vi.mocked(apiRequest).mockResolvedValue({ ...context, receipt: { order_id: 31 } });
  view(<SecondsPrincipalRefundAction helpers={helpers} record={{ ...record, status: 'refunded' }} />);
  await userEvent.click(screen.getByRole('button', { name: '退款凭证' }));
  await userEvent.click(await screen.findByRole('button', { name: '核对退款' }));
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: 'new command' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('仅可核对原未决退款请求');
  expect(vi.mocked(apiRequest).mock.calls.filter(([, init]) => init?.method === 'POST')).toHaveLength(1);
});

it('pins the key across response loss and remount and rejects malformed success receipts', async () => {
  vi.mocked(apiRequest).mockImplementation(async (_, init) => {
    if (init?.method === 'POST') return { order_id: 31 };
    return context;
  });
  const first = view(<SecondsPrincipalRefundAction helpers={helpers} record={record} />);
  await refund();
  expect(await screen.findByRole('alert')).toHaveTextContent('退款结果尚未确认');
  const original = vi.mocked(apiRequest).mock.calls.find(([, init]) => init?.method === 'POST')![1]!.body;
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toContain(JSON.parse(original as string).idempotency_key);
  first.unmount();
  mockRefund();
  view(<SecondsPrincipalRefundAction helpers={helpers} record={record} />);
  await refund();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(vi.mocked(apiRequest).mock.calls.filter(([, init]) => init?.method === 'POST')[1][1]!.body).toBe(original);
});

it('fails closed for malformed policy/context and unknown session storage', async () => {
  expect(() => parseSecondsRefundPolicy({ ...policy, enabled: true }, '4')).toThrow();
  expect(() => parseSecondsRefundContext({ ...context, checked_at: '2000' }, '31')).toThrow();
  mockRefund();
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('storage unavailable'); });
  view(<SecondsPrincipalRefundAction helpers={helpers} record={record} />);
  await refund();
  await screen.findByRole('alert');
  expect(vi.mocked(apiRequest).mock.calls.some(([, init]) => init?.method === 'POST')).toBe(false);
});
