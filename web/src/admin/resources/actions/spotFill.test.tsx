import { Toast } from '@douyinfe/semi-ui';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../api/client';
import { authStore } from '../../../auth/authStore';
import { FINANCIAL_COMMAND_STORAGE_KEY } from '../../../shared/idempotency';
import { AdminAccessProvider } from '../../access';
import { SpotFillAction } from './spotFill';

vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'), apiRequest: vi.fn()
}));

const helpers = { loadDetail: vi.fn(), openDetail: vi.fn(), reload: vi.fn() };
const buy = { id: '11', user_id: '101', pair_id: 'BTC-USDT', side: 'buy', price: '10', quantity: '2', filled_quantity: '0', status: 'open' };
const sell = { ...buy, id: '22', user_id: '102', side: 'sell' };
const result = { trade: { id: '33', buy_order_id: '11', sell_order_id: '22', price: '10', quantity: '1' } };
const endpoint = '/admin/api/v1/spot/fills';
const access = {
  admin_id: 7, username: 'operator', role_id: 1, role_name: 'operations', is_super_admin: false,
  permissions: ['spot.orders.read', 'spot.orders.write']
};

function view(permissions = access.permissions) {
  return render(<AdminAccessProvider access={{ ...access, permissions }}><SpotFillAction helpers={helpers} record={buy} /></AdminAccessProvider>);
}
async function draft() {
  await userEvent.click(screen.getByRole('button', { name: '手工成交' }));
  fireEvent.change(screen.getByLabelText('对手订单 ID'), { target: { value: '22' } });
  fireEvent.change(screen.getByLabelText('成交价格'), { target: { value: '10.00' } });
  fireEvent.change(screen.getByLabelText('成交数量'), { target: { value: '1.000' } });
}
async function preview() {
  await draft();
  await userEvent.click(screen.getByRole('button', { name: '核对双方订单' }));
  await screen.findByRole('region', { name: '买方订单预览' });
}
async function confirm() {
  await userEvent.click(screen.getByRole('button', { name: '确认手工成交' }));
  expect(screen.getByRole('button', { name: '确认' })).toBeDisabled();
  fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '  checked orders  ' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
}
function posts() {
  return vi.mocked(apiRequest).mock.calls.filter(([path]) => path === endpoint)
    .map(([, init]) => JSON.parse(init!.body as string) as Record<string, unknown>);
}
function responses(post: () => Promise<unknown> = async () => result) {
  vi.mocked(apiRequest).mockImplementation(async (path) => {
    if (path.endsWith('/11')) return buy;
    if (path.endsWith('/22')) return sell;
    return post();
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  sessionStorage.clear();
  authStore.setSession({ accessToken: 'test', refreshToken: 'refresh', scope: 'admin', subject: 'admin:7' });
  vi.spyOn(Toast, 'success').mockImplementation(() => 'success');
  responses();
});
afterEach(() => vi.restoreAllMocks());

it('gates the action by both existing resource read and write permissions', () => {
  const rendered = view(['spot.orders.read']);
  expect(screen.queryByRole('button', { name: '手工成交' })).not.toBeInTheDocument();
  rendered.unmount();
  view(['spot.orders.write']);
  expect(screen.queryByRole('button', { name: '手工成交' })).not.toBeInTheDocument();
  expect(apiRequest).not.toHaveBeenCalled();
});

it('reads both orders without mutation, requires reason, sends decimal text and refreshes on success', async () => {
  view();
  await preview();
  expect(apiRequest).toHaveBeenCalledTimes(2);
  expect(posts()).toEqual([]);
  expect(screen.getByRole('region', { name: '卖方订单预览' })).toHaveTextContent('102');
  await confirm();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(posts()).toEqual([{
    buy_order_id: '11', sell_order_id: '22', price: '10', quantity: '1', reason: 'checked orders',
    idempotency_key: expect.stringMatching(/^admin-spot-fill-/)
  }]);
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toBeNull();
});

it('retains a stable key after response loss and remount even when both orders are now filled', async () => {
  responses(async () => { throw new Error('response lost'); });
  const rendered = view();
  await preview();
  await confirm();
  await screen.findByRole('alert');
  const first = posts()[0];
  expect(helpers.reload).not.toHaveBeenCalled();
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toContain(first.idempotency_key);
  rendered.unmount();
  vi.mocked(apiRequest).mockImplementation(async (path) => {
    if (path.endsWith('/11')) return { ...buy, status: 'filled', filled_quantity: '2' };
    if (path.endsWith('/22')) return { ...sell, status: 'filled', filled_quantity: '2' };
    return result;
  });
  view();
  await preview();
  await confirm();
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
  expect(posts()[1]).toEqual(first);
});

it('invalidates preview on edits and ignores a late response after draft replacement', async () => {
  let resolveBuy!: (value: unknown) => void;
  vi.mocked(apiRequest).mockImplementation(async (path) =>
    path.endsWith('/11') ? new Promise((resolve) => { resolveBuy = resolve; }) : sell
  );
  view();
  await draft();
  await userEvent.click(screen.getByRole('button', { name: '核对双方订单' }));
  fireEvent.change(screen.getByLabelText('成交数量'), { target: { value: '2' } });
  resolveBuy(buy);
  await waitFor(() => expect(screen.getByRole('button', { name: '核对双方订单' })).not.toBeDisabled());
  expect(screen.queryByRole('button', { name: '确认手工成交' })).not.toBeInTheDocument();
  expect(posts()).toEqual([]);
});

it.each([
  { ...sell, pair_id: 'ETH-USDT' }, { ...sell, quantity: 2 }, { ...sell, side: 'buy' }
])('fails closed for incompatible or malformed preview %j', async (badSell) => {
  vi.mocked(apiRequest).mockImplementation(async (path) => path.endsWith('/11') ? buy : badSell);
  view();
  await draft();
  await userEvent.click(screen.getByRole('button', { name: '核对双方订单' }));
  await screen.findByRole('alert');
  expect(screen.queryByRole('button', { name: '确认手工成交' })).not.toBeInTheDocument();
  expect(posts()).toEqual([]);
});

it('blocks a new fill on terminal orders and does not generate a command key', async () => {
  vi.mocked(apiRequest).mockImplementation(async (path) =>
    ({ ...(path.endsWith('/11') ? buy : sell), status: 'filled', filled_quantity: '2' })
  );
  view();
  await preview();
  await confirm();
  await screen.findByRole('alert');
  expect(posts()).toEqual([]);
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toBeNull();
});

it('keeps malformed success responses uncertain and does not refresh', async () => {
  responses(async () => ({}));
  view();
  await preview();
  await confirm();
  await screen.findByRole('alert');
  expect(helpers.reload).not.toHaveBeenCalled();
  expect(sessionStorage.getItem(FINANCIAL_COMMAND_STORAGE_KEY)).toContain(posts()[0].idempotency_key);
});

it('is single-flight and freezes the draft while a financial request is pending', async () => {
  let resolvePost!: (value: unknown) => void;
  responses(() => new Promise((resolve) => { resolvePost = resolve; }));
  view();
  await preview();
  await confirm();
  fireEvent.click(screen.getByRole('button', { name: '确认' }));
  expect(posts()).toHaveLength(1);
  expect(screen.getByLabelText('成交数量')).toBeDisabled();
  expect(screen.getByRole('button', { name: '取消' })).toBeDisabled();
  resolvePost(result);
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
});

it('fails closed before POST when pending command persistence is unavailable', async () => {
  view();
  await preview();
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('storage unavailable'); });
  await confirm();
  await screen.findByRole('alert');
  expect(posts()).toEqual([]);
});

it('does not reuse an uncertain key when the operator changes the reason', async () => {
  responses(async () => { throw new Error('response lost'); });
  view();
  await preview();
  await confirm();
  await screen.findByRole('alert');
  fireEvent.change(screen.getByLabelText('操作原因'), { target: { value: 'different intent' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(posts()).toHaveLength(2));
  expect(posts()[1].idempotency_key).not.toBe(posts()[0].idempotency_key);
});
