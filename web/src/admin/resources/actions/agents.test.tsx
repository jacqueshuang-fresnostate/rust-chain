import { Toast } from '@douyinfe/semi-ui';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../api/client';
import { AgentCommissionBatchActions } from './agents';

vi.mock('../../../api/client', async () => ({ ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'), apiRequest: vi.fn() }));
const requestMock = vi.mocked(apiRequest);

function helpers(ids: number[]) {
  return { clearSelection: vi.fn(), reload: vi.fn(), selectedRows: ids.map((id) => ({ id, status: 'pending' })) };
}

beforeEach(() => {
  requestMock.mockReset();
  vi.spyOn(Toast, 'error').mockImplementation(() => 'error');
  vi.spyOn(Toast, 'success').mockImplementation(() => 'success');
});
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });

describe('commission batch confirmation scope', () => {
  it('pins the original targets and never re-enables an invalidated confirmation', async () => {
    const view = render(<AgentCommissionBatchActions helpers={helpers([1, 2])} />);
    await userEvent.click(screen.getByRole('button', { name: '批量结算' }));
    await screen.findByRole('button', { name: '确认批量结算' });
    view.rerender(<AgentCommissionBatchActions helpers={helpers([3])} />);
    expect(screen.getByText('批量结算（2 条）')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '确认批量结算' })).toBeDisabled();
    view.rerender(<AgentCommissionBatchActions helpers={helpers([1, 2])} />);
    expect(screen.getByRole('button', { name: '确认批量结算' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '确认批量结算' }));
    expect(requestMock).not.toHaveBeenCalled();
  });

  it('single-flights confirmation and blocks cancel/reason changes until POST settles', async () => {
    let resolve!: (value: unknown) => void;
    requestMock.mockReturnValueOnce(new Promise((yes) => { resolve = yes; }));
    const current = helpers([2, 1]);
    render(<AgentCommissionBatchActions helpers={current} />);
    await userEvent.click(screen.getByRole('button', { name: '批量驳回' }));
    fireEvent.change(await screen.findByLabelText('批量操作原因'), { target: { value: '  checked  ' } });
    const confirm = screen.getByRole('button', { name: '确认批量驳回' });
    act(() => { fireEvent.click(confirm); fireEvent.click(confirm); });
    expect(requestMock).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled();
    expect(screen.getByLabelText('批量操作原因')).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    expect(screen.getByLabelText('批量操作原因')).toHaveValue('  checked  ');
    expect(requestMock).toHaveBeenCalledWith('/admin/api/v1/agent-commissions/batch-status', {
      method: 'POST', body: JSON.stringify({ ids: [2, 1], status: 'rejected', reason: 'checked' })
    });
    await act(async () => { resolve({ results: [{ id: 2, status: 'ok', error: null }, { id: 1, status: 'ok', error: null }] }); });
    await waitFor(() => expect(screen.queryByLabelText('批量操作原因')).not.toBeInTheDocument());
    expect(current.clearSelection).toHaveBeenCalledTimes(1);
    expect(current.reload).toHaveBeenCalledTimes(1);
  });

  it('retains the original reason and scope after POST failure for explicit retry', async () => {
    requestMock.mockRejectedValueOnce(new Error('batch failed')).mockResolvedValueOnce({ results: [{ id: 1, status: 'ok', error: null }] });
    const current = helpers([1]);
    render(<AgentCommissionBatchActions helpers={current} />);
    await userEvent.click(screen.getByRole('button', { name: '批量结算' }));
    fireEvent.change(await screen.findByLabelText('批量操作原因'), { target: { value: 'reviewed' } });
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole('button', { name: '确认批量结算' }));
    // Semi Modal debounces an explicit second click for 100 ms; advance that actual UI contract.
    await act(async () => { await vi.advanceTimersByTimeAsync(101); });
    expect(Toast.error).toHaveBeenCalledWith(expect.stringContaining('batch failed'));
    expect(screen.getByLabelText('批量操作原因')).toHaveValue('reviewed');
    expect(current.reload).not.toHaveBeenCalled();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '确认批量结算' })); });
    expect(requestMock).toHaveBeenCalledTimes(2);
    expect(requestMock.mock.calls[1]).toEqual(requestMock.mock.calls[0]);
  });

  it('does not open an empty batch and rejects oversized selections', async () => {
    const view = render(<AgentCommissionBatchActions helpers={helpers([])} />);
    expect(screen.getByRole('button', { name: '批量结算' })).toBeDisabled();
    view.rerender(<AgentCommissionBatchActions helpers={helpers(Array.from({ length: 201 }, (_, i) => i + 1))} />);
    await userEvent.click(screen.getByRole('button', { name: '批量结算' }));
    expect(Toast.error).toHaveBeenCalledWith('单次最多批量处理 200 条');
    expect(screen.queryByRole('button', { name: '确认批量结算' })).not.toBeInTheDocument();
    expect(requestMock).not.toHaveBeenCalled();
  });
});
