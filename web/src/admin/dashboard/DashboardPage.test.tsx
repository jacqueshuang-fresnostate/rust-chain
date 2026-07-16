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
      admin_actions_24h: 1234,
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
      active_pairs: 1234,
      disabled_pairs: 1,
      external_pairs: 45,
      feed_needs_reload: false,
      feed_providers: ['bitget', 'htx'],
      feed_runtime_status: 'success',
      feed_symbols: ['BTC-USDT', 'ETH-USDT'],
      strategy_pairs: 67
    },
    products: {
      earn_active_subscriptions: 890,
      earn_maturing_24h: 12,
      margin_liquidated_24h: 34,
      margin_open_positions: 567,
      seconds_open_orders: 1234
    },
    risk: {
      blocked_events_24h: 34,
      dead_letter_inbox_events: 56,
      pending_outbox_events: 1234,
      retry_inbox_events: 78,
      risk_events_24h: 12
    },
    trading: {
      convert_completed_24h: 10,
      convert_pending_orders: 1234,
      spot_open_orders: 5678,
      spot_trades_24h: 9012
    },
    users: {
      active: 2345,
      new_24h: 678,
      total: 123456
    },
    wallet: {
      active_assets: 1234,
      custody_status: 'not_configured',
      non_zero_accounts: 7890,
      pending_deposits: 34,
      pending_unlocks: 12,
      pending_withdrawals: 56,
      wallet_accounts: 4567
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
    expect(screen.getByText('123,456.00')).toBeInTheDocument();
    expect(screen.getByText('活跃 2,345.00，24h 新增 678.00')).toBeInTheDocument();
    expect(screen.getByText('活跃资产')).toBeInTheDocument();
    expect(screen.getAllByText('1,234.00')).toHaveLength(2);
    expect(screen.getByText('钱包账户 4,567.00，非零账户 7,890.00')).toBeInTheDocument();
    expect(screen.getByText('活跃交易对')).toBeInTheDocument();
    expect(screen.getByText('外部行情 45.00，策略行情 67.00')).toBeInTheDocument();
    expect(screen.getByText('现货挂单')).toBeInTheDocument();
    expect(screen.getByText('5,678.00')).toBeInTheDocument();
    expect(screen.getByText('24h 成交')).toBeInTheDocument();
    expect(screen.getByText('9,012.00')).toBeInTheDocument();
    expect(screen.getByText('24h 成交 9,012.00')).toBeInTheDocument();
    expect(screen.getByText('闪兑待处理 1,234.00')).toBeInTheDocument();
    expect(screen.getByText('事件积压')).toBeInTheDocument();
    expect(screen.getByText('1,368.00')).toBeInTheDocument();
    expect(screen.getByText('风控事件 12.00，阻断 34.00')).toBeInTheDocument();
    expect(screen.getByText(/行情订阅/)).toBeInTheDocument();
    expect(screen.getByText(/bitget, htx/)).toBeInTheDocument();
    expect(screen.getByText(/BTC-USDT, ETH-USDT/)).toBeInTheDocument();
    expect(screen.getByText('风控 / 事件积压')).toBeInTheDocument();
    expect(screen.getByText(/链上托管未接入生产监听/)).toBeInTheDocument();
    expect(screen.getByText('待解禁：12.00')).toBeInTheDocument();
    expect(screen.getByText('待充值确认：34.00')).toBeInTheDocument();
    expect(screen.getByText('待提现处理：56.00')).toBeInTheDocument();
    expect(screen.getByText('秒合约未结算订单：1,234.00')).toBeInTheDocument();
    expect(screen.getByText('杠杆持仓：567.00')).toBeInTheDocument();
    expect(screen.getByText('24h 强平：34.00')).toBeInTheDocument();
    expect(screen.getByText('Earn 生效申购：890.00')).toBeInTheDocument();
    expect(screen.getByText('24h 到期 Earn：12.00')).toBeInTheDocument();
    expect(screen.getByText('24h 风控事件：12.00')).toBeInTheDocument();
    expect(screen.getByText('24h 阻断事件：34.00')).toBeInTheDocument();
    expect(screen.getByText('Outbox 待发布：1,234.00')).toBeInTheDocument();
    expect(screen.getByText('Inbox 重试：78.00')).toBeInTheDocument();
    expect(screen.getByText('Inbox 死信：56.00')).toBeInTheDocument();
    expect(screen.queryByText('最新审计动作')).not.toBeInTheDocument();
    expect(screen.queryByText('24h 管理动作：1,234.00')).not.toBeInTheDocument();
    expect(screen.queryByText('asset.create')).not.toBeInTheDocument();
    expect(screen.queryByText('asset #9')).not.toBeInTheDocument();
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
