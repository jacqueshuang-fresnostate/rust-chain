import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest } from '../../api/client';
import { MarketStrategyRecoverySheet, recoveryPreviewExpired } from './MarketStrategyRecoverySheet';

vi.mock('../../api/client', async (original) => ({ ...await original<typeof import('../../api/client')>(), apiRequest: vi.fn() }));
const request = vi.mocked(apiRequest);
const gap = { range_start: 1_700_000_000_000, range_end: 1_700_000_060_000, one_minute_count: 1 };
const gaps = { config_version: 1, strategy_id: 91, total_1m_count: 1, gaps: [gap] };

beforeEach(() => {
  request.mockReset();
  if (!Range.prototype.getBoundingClientRect) Range.prototype.getBoundingClientRect = () => new DOMRect(0, 0, 0, 0);
});

describe('strategy recovery lifecycle', () => {
  it('checks the expiry boundary and invalid deadlines', () => {
    expect(recoveryPreviewExpired(100, 99)).toBe(false);
    expect(recoveryPreviewExpired(100, 100)).toBe(true);
    expect(recoveryPreviewExpired(Number.NaN, 100)).toBe(true);
  });

  it('keeps an expired preview readable but blocks every execution request', async () => {
    request.mockImplementation(async (path) => {
      if (path.endsWith('/kline-gaps')) return gaps;
      if (path.endsWith('/preview')) return { ...gap, strategy_id: 91, config_version: 1, expires_at: Date.now() - 1, preview_token: 'expired-fixture', aggregate_intervals: ['5m'], first_price: '1', last_price: '2', samples: [] };
      return { jobs: [], total: 0 };
    });
    const user = userEvent.setup();
    render(<MarketStrategyRecoverySheet strategyId="91" />);
    await user.click(screen.getByRole('button', { name: '检测缺口/补偿K线（策略91）' }));
    await user.click(screen.getByRole('button', { name: '重新检测K线缺口' }));
    await user.click(await screen.findByRole('button', { name: `预览缺口${gap.range_start}` }));
    expect(await screen.findByText('补偿预览')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('补偿原因'), { target: { value: 'fixture reason' } });
    expect(await screen.findByText('补偿预览已过期，请重新预览后再确认执行')).toBeInTheDocument();
    const execute = screen.getByRole('button', { name: '确认执行K线补偿' });
    expect(execute).toBeDisabled();
    await user.click(execute);
    expect(request.mock.calls.some(([path]) => path.endsWith('/execute'))).toBe(false);
  });

  it('does not revive a closed sheet with a late gap response', async () => {
    let resolveGap: (value: unknown) => void = () => undefined;
    request.mockImplementation(async (path) => path.endsWith('/kline-gaps') ? new Promise((resolve) => { resolveGap = resolve; }) : { jobs: [], total: 0 });
    const user = userEvent.setup();
    render(<MarketStrategyRecoverySheet strategyId="91" />);
    await user.click(screen.getByRole('button', { name: '检测缺口/补偿K线（策略91）' }));
    await user.click(screen.getByRole('button', { name: '重新检测K线缺口' }));
    await waitFor(() => expect(request.mock.calls.some(([path]) => path.endsWith('/kline-gaps'))).toBe(true));
    await user.click(document.querySelector('.semi-sidesheet-close') as HTMLElement);
    await act(async () => resolveGap(gaps));
    await user.click(screen.getByRole('button', { name: '检测缺口/补偿K线（策略91）' }));
    await screen.findByText('暂无补偿任务。');
    expect(screen.queryByText('缺口范围（共 1 根 1m）')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '重新检测K线缺口' })).toBeEnabled();
  });
});
