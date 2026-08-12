import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { MarketStrategyActions } from './MarketStrategyActions';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const listAdminResourceMock = vi.mocked(listAdminResource);
const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function stubResizeObserver() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  if (descriptor?.configurable === false) {
    if ('writable' in descriptor && descriptor.writable) {
      (globalThis as typeof globalThis & { ResizeObserver: typeof ResizeObserverMock }).ResizeObserver = ResizeObserverMock;
    }
    return;
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
}

describe('MarketStrategyActions', () => {
  beforeEach(() => {
    stubResizeObserver();
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
    listAdminResourceMock.mockResolvedValue({
      rows: [
        {
          id: 91,
          pair_id: 21,
          symbol: 'BTC-USDT',
          strategy_type: 'price_path',
          start_price: '1.000000000000000000',
          target_price: '2.000000000000000000',
          status: 'paused',
          run_status: 'paused',
          created_at: 1_775_027_600_000
        }
      ],
      raw: { strategies: [] }
    });
  });

  it('renders strategy actions as a resource table page', async () => {
    render(<MarketStrategyActions />);

    expect(await screen.findByText('行情策略动作')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.getByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检测缺口/补偿K线（策略91）' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '启用' })).toBeInTheDocument();
    expect(screen.queryByText('更新策略状态')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
  });

  it('loads the strategy detail before editing so configured nodes are retained', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockResolvedValueOnce({
      id: 91,
      pair_id: 21,
      strategy_type: 'price_path',
      start_price: '1',
      target_price: '2',
      start_time: 1_775_027_600_000,
      end_time: 1_775_031_200_000,
      volatility: '0.01',
      volume_min: '10',
      volume_max: '20',
      status: 'paused',
      nodes: [
        {
          sequence_no: 0,
          target_time: 1_775_029_400_000,
          target_type: 'absolute_price',
          target_value: '1.5',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: '10',
          volume_max: '20'
        }
      ]
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '修改' }));

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/91'));
    expect(await screen.findByText('节点1')).toBeInTheDocument();
    expect(screen.getByDisplayValue('1.5')).toBeInTheDocument();
  });

  it('keeps create submission disabled until the strategy range is valid', async () => {
    const user = userEvent.setup();
    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '创建策略' }));

    const sheet = (await screen.findByText('创建策略', { selector: '.semi-sidesheet-title' })).closest('.semi-sidesheet-inner') as HTMLElement;
    fireEvent.change(within(sheet).getByLabelText('交易对ID'), { target: { value: '21' } });
    fireEvent.change(within(sheet).getByLabelText('起始价'), { target: { value: '1' } });
    fireEvent.change(within(sheet).getByLabelText('目标价'), { target: { value: '2' } });
    fireEvent.change(within(sheet).getByLabelText('开始时间'), { target: { value: '2026-08-12T10:00' } });
    fireEvent.change(within(sheet).getByLabelText('结束时间'), { target: { value: '2026-08-12T10:00' } });

    const submit = within(sheet).getByRole('button', { name: '提交创建策略' });
    expect(submit).toBeDisabled();

    fireEvent.change(within(sheet).getByLabelText('结束时间'), { target: { value: '2026-08-12T11:00' } });
    expect(submit).toBeEnabled();
  });

  it('keeps edit submission disabled for boundary, duplicate, or descending node times', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockResolvedValueOnce({
      id: 91,
      pair_id: 21,
      strategy_type: 'price_path',
      start_price: '1',
      target_price: '2',
      start_time: new Date('2026-08-12T10:00').getTime(),
      end_time: new Date('2026-08-12T11:00').getTime(),
      volatility: '0.01',
      volume_min: '10',
      volume_max: '20',
      status: 'paused',
      nodes: [
        {
          sequence_no: 0,
          target_time: new Date('2026-08-12T10:20').getTime(),
          target_type: 'absolute_price',
          target_value: '1.2',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: null,
          volume_max: null
        },
        {
          sequence_no: 1,
          target_time: new Date('2026-08-12T10:40').getTime(),
          target_type: 'absolute_price',
          target_value: '1.4',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: null,
          volume_max: null
        }
      ]
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '修改' }));
    const sheet = (await screen.findByText('修改行情策略', { selector: '.semi-sidesheet-title' })).closest('.semi-sidesheet-inner') as HTMLElement;
    const submit = within(sheet).getByRole('button', { name: '提交修改' });
    const nodeTimes = within(sheet).getAllByLabelText(/节点\d+目标时间/);

    expect(submit).toBeEnabled();

    fireEvent.change(nodeTimes[0], { target: { value: '2026-08-12T10:00' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[0], { target: { value: '2026-08-12T10:30' } });
    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:30' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:20' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:45' } });
    expect(submit).toBeEnabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T11:00' } });
    expect(submit).toBeDisabled();
  });
});
