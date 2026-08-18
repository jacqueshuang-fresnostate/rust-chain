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

function dashboardResponse({
  environment = 'test',
  latestActions = [
    {
      action: 'asset.create',
      admin_id: 1,
      created_at: 1_735_732_800_000,
      id: 123,
      target_id: '9',
      target_type: 'asset'
    }
  ]
}: {
  environment?: string;
  latestActions?: Array<{
    action: string;
    admin_id: number;
    created_at: number;
    id: number;
    target_id: string;
    target_type: string;
  }>;
} = {}) {
  return {
    audit: {
      admin_actions_24h: 1234,
      latest_actions: latestActions
    },
    environment,
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
    expect(screen.getByText('123,456')).toBeInTheDocument();
    expect(screen.getByText('活跃 2,345，24h 新增 678')).toBeInTheDocument();
    expect(screen.getByText('活跃资产')).toBeInTheDocument();
    expect(screen.getAllByText('1,234')).toHaveLength(2);
    expect(screen.getByText('钱包账户 4,567，非零账户 7,890')).toBeInTheDocument();
    expect(screen.getByText('活跃交易对')).toBeInTheDocument();
    expect(screen.getByText('外部行情 45，策略行情 67')).toBeInTheDocument();
    expect(screen.getByText('现货挂单')).toBeInTheDocument();
    expect(screen.getByText('5,678')).toBeInTheDocument();
    expect(screen.getByText('24h 成交')).toBeInTheDocument();
    expect(screen.getByText('9,012')).toBeInTheDocument();
    expect(screen.getByText('24h 成交 9,012')).toBeInTheDocument();
    expect(screen.getByText('闪兑待处理 1,234')).toBeInTheDocument();
    expect(screen.getByText('事件积压')).toBeInTheDocument();
    expect(screen.getByText('1,368')).toBeInTheDocument();
    expect(screen.getByText('风控事件 12，阻断 34')).toBeInTheDocument();
    expect(screen.getByText(/行情订阅/)).toBeInTheDocument();
    expect(screen.getByText(/bitget, htx/)).toBeInTheDocument();
    expect(screen.getByText(/BTC-USDT, ETH-USDT/)).toBeInTheDocument();
    expect(screen.getByText('风控 / 事件积压')).toBeInTheDocument();
    expect(screen.getByText(/链上托管未接入运行监听/)).toBeInTheDocument();
    expect(screen.getByText('待解禁：12')).toBeInTheDocument();
    expect(screen.getByText('待充值确认：34')).toBeInTheDocument();
    expect(screen.getByText('待提现处理：56')).toBeInTheDocument();
    expect(screen.getByText('秒合约未结算订单：1,234')).toBeInTheDocument();
    expect(screen.getByText('杠杆持仓：567')).toBeInTheDocument();
    expect(screen.getByText('24h 强平：34')).toBeInTheDocument();
    expect(screen.getByText('Earn 生效申购：890')).toBeInTheDocument();
    expect(screen.getByText('24h 到期 Earn：12')).toBeInTheDocument();
    expect(screen.getByText('24h 风控事件：12')).toBeInTheDocument();
    expect(screen.getByText('24h 阻断事件：34')).toBeInTheDocument();
    expect(screen.getByText('Outbox 待发布：1,234')).toBeInTheDocument();
    expect(screen.getByText('Inbox 重试：78')).toBeInTheDocument();
    expect(screen.getByText('Inbox 死信：56')).toBeInTheDocument();
    expect(screen.getByText('测试环境')).toBeInTheDocument();
    expect(screen.queryByText('生产环境')).not.toBeInTheDocument();
    expect(screen.getByText('最近配置与运营动作')).toBeInTheDocument();
    expect(screen.getByText('24 小时管理操作：1,234')).toBeInTheDocument();
    expect(screen.getByText('创建资产')).toBeInTheDocument();
    expect(screen.getByText('目标：资产 #9')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看全部审计日志' })).toHaveAttribute(
      'href',
      '/admin/audit-logs'
    );
    expect(screen.queryByText('asset.create')).not.toBeInTheDocument();
    expect(screen.queryByText('asset #9')).not.toBeInTheDocument();
  });

  it.each([
    ['production', '生产环境', 'red'],
    ['staging', '预发布环境', 'orange'],
    ['test', '测试环境', 'light-blue'],
    ['development', '开发环境', 'grey']
  ])('maps %s to a Chinese semantic environment tag', async (environment, label, semanticColor) => {
    apiRequestMock.mockResolvedValueOnce(dashboardResponse({ environment }));

    render(<DashboardPage />);

    const labelElement = await screen.findByText(label);
    const tag = labelElement.closest('[data-environment]');
    expect(tag).toHaveAttribute('data-environment', environment);
    expect(tag).toHaveAttribute('data-semantic-color', semanticColor);
  });

  it('shows explicit initial loading and empty audit states', async () => {
    apiRequestMock.mockResolvedValueOnce(dashboardResponse({ latestActions: [] }));

    render(<DashboardPage />);

    expect(screen.getByText('正在加载总览数据…')).toBeInTheDocument();
    expect(await screen.findByText('暂无最近配置或运营动作')).toBeInTheDocument();
    expect(screen.getByText('24 小时管理操作：1,234')).toBeInTheDocument();
  });

  it('shows load failure and retries with refresh button', async () => {
    const user = userEvent.setup();
    const requestError = new Error('network down\n    at private_backend_path.rs:42');
    requestError.stack = 'SENSITIVE_BACKEND_STACK';
    apiRequestMock.mockRejectedValueOnce(requestError).mockResolvedValueOnce(dashboardResponse());

    render(<DashboardPage />);

    expect(await screen.findByText(/加载失败：network down/)).toBeInTheDocument();
    expect(screen.queryByText(/private_backend_path/)).not.toBeInTheDocument();
    expect(screen.queryByText(/SENSITIVE_BACKEND_STACK/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '刷新总览' }));

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('用户总数')).toBeInTheDocument();
  });
});
