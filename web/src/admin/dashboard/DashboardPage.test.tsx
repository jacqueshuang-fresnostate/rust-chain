import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { DashboardPage } from './DashboardPage';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const apiRequestMock = vi.mocked(apiRequest);

function dashboardResponse() {
  return {
    audit: {
      admin_actions_24h: 9,
      latest_actions: [
        {
          action: 'asset.create',
          admin_id: 1,
          created_at: 1_735_732_800_000,
          id: 123,
          target_id: '9',
          target_type: 'asset'
        }
      ]
    },
    generated_at: 1_735_732_800_000,
    market: {
      active_pairs: 6,
      disabled_pairs: 1,
      external_pairs: 4,
      feed_needs_reload: false,
      feed_providers: ['bitget', 'htx'],
      feed_runtime_status: 'success',
      feed_symbols: ['BTC-USDT', 'ETH-USDT'],
      strategy_pairs: 2
    },
    products: {
      earn_active_subscriptions: 8,
      earn_maturing_24h: 2,
      margin_liquidated_24h: 1,
      margin_open_positions: 3,
      seconds_open_orders: 5
    },
    risk: {
      blocked_events_24h: 1,
      dead_letter_inbox_events: 0,
      pending_outbox_events: 0,
      retry_inbox_events: 0,
      risk_events_24h: 6
    },
    trading: {
      convert_completed_24h: 10,
      convert_pending_orders: 2,
      spot_open_orders: 12,
      spot_trades_24h: 31
    },
    users: {
      active: 118,
      new_24h: 4,
      total: 120
    },
    wallet: {
      active_assets: 8,
      custody_status: 'not_configured',
      non_zero_accounts: 92,
      pending_deposits: 0,
      pending_unlocks: 3,
      pending_withdrawals: 0,
      wallet_accounts: 240
    }
  };
}

describe('DashboardPage', () => {
  beforeEach(() => {
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue(dashboardResponse());
  });

  it('loads exchange operational dashboard metrics', async () => {
    render(<DashboardPage />);

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/dashboard'));
    expect(await screen.findByText('用户总数')).toBeInTheDocument();
    expect(screen.getByText('120')).toBeInTheDocument();
    expect(screen.getByText('活跃资产')).toBeInTheDocument();
    expect(screen.getByText('8')).toBeInTheDocument();
    expect(screen.getByText('现货挂单')).toBeInTheDocument();
    expect(screen.getByText('12')).toBeInTheDocument();
    expect(screen.getByText('24h 成交')).toBeInTheDocument();
    expect(screen.getByText('31')).toBeInTheDocument();
    expect(screen.getByText(/行情订阅/)).toBeInTheDocument();
    expect(screen.getByText(/bitget, htx/)).toBeInTheDocument();
    expect(screen.getByText(/BTC-USDT, ETH-USDT/)).toBeInTheDocument();
    expect(screen.getByText('风控 / 事件积压')).toBeInTheDocument();
    expect(screen.getByText(/链上托管未接入生产监听/)).toBeInTheDocument();
    expect(screen.getByText('asset.create')).toBeInTheDocument();
    expect(screen.getByText('asset #9')).toBeInTheDocument();
  });

  it('shows load failure and retries with refresh button', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockRejectedValueOnce(new Error('network down')).mockResolvedValueOnce(dashboardResponse());

    render(<DashboardPage />);

    expect(await screen.findByText(/加载失败：network down/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '刷新总览' }));

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('用户总数')).toBeInTheDocument();
  });
});
