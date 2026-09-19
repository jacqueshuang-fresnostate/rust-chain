import { Toast } from '@douyinfe/semi-ui';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { apiRequest } from '../../../api/client';
import { SecondsOrderRowActions } from './secondsContract';

vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'),
  apiRequest: vi.fn()
}));

beforeEach(() => {
  vi.mocked(apiRequest).mockReset();
  vi.spyOn(Toast, 'success').mockImplementation(() => 'success');
  vi.spyOn(Toast, 'error').mockImplementation(() => 'error');
});
afterEach(() => vi.restoreAllMocks());

it('review recovery sends only automatic evidence mode and retains the reason after rejection', async () => {
  const helpers = { loadDetail: vi.fn(), openDetail: vi.fn(), reload: vi.fn() };
  vi.mocked(apiRequest).mockRejectedValueOnce(new Error('history pending'));
  render(<SecondsOrderRowActions helpers={helpers} record={{ id: 17, status: 'manual_review' }} />);
  expect(screen.queryByRole('button', { name: '结算赢' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '结算输' })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '恢复结算' }));
  fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '  checked archive  ' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(apiRequest).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/orders/17/settle', {
    method: 'POST', body: JSON.stringify({ result: 'auto', reason: 'checked archive' })
  }));
  await screen.findByRole('alert');
  expect(screen.getByLabelText('操作原因')).toHaveValue('  checked archive  ');
  expect(helpers.reload).not.toHaveBeenCalled();
});

it('successful recovery refreshes the authoritative order', async () => {
  const helpers = { loadDetail: vi.fn(), openDetail: vi.fn(), reload: vi.fn() };
  vi.mocked(apiRequest).mockResolvedValue({ order: { id: 17, status: 'settled', result: 'loss' }, payout_amount: '0' });
  render(<SecondsOrderRowActions helpers={helpers} record={{ id: 17, status: 'manual_review' }} />);
  await userEvent.click(screen.getByRole('button', { name: '恢复结算' }));
  fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: 'verified event-time price' } });
  await userEvent.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledTimes(1));
});
