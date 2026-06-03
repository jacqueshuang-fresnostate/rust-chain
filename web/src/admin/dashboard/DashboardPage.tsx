import { Banner, Button, Card, Space, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { formatAdminNumber } from '../../shared/numberFormat';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

const { Text, Title } = Typography;

type DashboardResponse = {
  audit: {
    admin_actions_24h: number;
    latest_actions: Array<{
      action: string;
      admin_id: number;
      created_at: number;
      id: number;
      target_id: string;
      target_type: string;
    }>;
  };
  generated_at: number;
  market: {
    active_pairs: number;
    disabled_pairs: number;
    external_pairs: number;
    feed_needs_reload: boolean;
    feed_providers: string[];
    feed_runtime_status: string;
    feed_symbols: string[];
    strategy_pairs: number;
  };
  products: {
    earn_active_subscriptions: number;
    earn_maturing_24h: number;
    margin_liquidated_24h: number;
    margin_open_positions: number;
    seconds_open_orders: number;
  };
  risk: {
    blocked_events_24h: number;
    dead_letter_inbox_events: number;
    pending_outbox_events: number;
    retry_inbox_events: number;
    risk_events_24h: number;
  };
  trading: {
    convert_completed_24h: number;
    convert_pending_orders: number;
    spot_open_orders: number;
    spot_trades_24h: number;
  };
  users: {
    active: number;
    new_24h: number;
    total: number;
  };
  wallet: {
    active_assets: number;
    custody_status: string;
    non_zero_accounts: number;
    pending_deposits: number;
    pending_unlocks: number;
    pending_withdrawals: number;
    wallet_accounts: number;
  };
};

type KpiCard = {
  description: string;
  label: string;
  value: string;
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '加载失败';
}

function joinList(values: string[]) {
  return values.length ? values.join(', ') : '-';
}

function custodyText(status: string) {
  if (status === 'not_configured') {
    return '链上托管未接入生产监听';
  }
  return status;
}

function displayNumber(value: number) {
  return formatAdminNumber(value) ?? String(value);
}

export function DashboardPage() {
  const [dashboard, setDashboard] = useState<DashboardResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const kpis = useMemo<KpiCard[]>(() => {
    if (!dashboard) {
      return [];
    }

    return [
      { label: '用户总数', value: displayNumber(dashboard.users.total), description: `活跃 ${displayNumber(dashboard.users.active)}，24h 新增 ${displayNumber(dashboard.users.new_24h)}` },
      { label: '活跃资产', value: displayNumber(dashboard.wallet.active_assets), description: `钱包账户 ${displayNumber(dashboard.wallet.wallet_accounts)}，非零账户 ${displayNumber(dashboard.wallet.non_zero_accounts)}` },
      { label: '活跃交易对', value: displayNumber(dashboard.market.active_pairs), description: `外部行情 ${displayNumber(dashboard.market.external_pairs)}，策略行情 ${displayNumber(dashboard.market.strategy_pairs)}` },
      { label: '现货挂单', value: displayNumber(dashboard.trading.spot_open_orders), description: `24h 成交 ${displayNumber(dashboard.trading.spot_trades_24h)}` },
      { label: '24h 成交', value: displayNumber(dashboard.trading.spot_trades_24h), description: `闪兑待处理 ${displayNumber(dashboard.trading.convert_pending_orders)}` },
      {
        label: '事件积压',
        value: displayNumber(dashboard.risk.pending_outbox_events + dashboard.risk.retry_inbox_events + dashboard.risk.dead_letter_inbox_events),
        description: `风控事件 ${displayNumber(dashboard.risk.risk_events_24h)}，阻断 ${displayNumber(dashboard.risk.blocked_events_24h)}`
      }
    ];
  }, [dashboard]);

  async function loadDashboard() {
    setLoading(true);
    try {
      const response = await apiRequest<DashboardResponse>('/admin/api/v1/dashboard');
      setDashboard(response);
      setError(null);
    } catch (requestError) {
      setError(errorMessage(requestError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadDashboard();
  }, []);

  return (
    <main className="exchange-page admin-dashboard-page">
      <PageHeader title="总览仪表盘" />
      <div className="admin-dashboard-toolbar">
        <Space wrap>
          <Button loading={loading} onClick={loadDashboard} theme="solid" type="primary">
            刷新总览
          </Button>
          <Text type="secondary">生成时间：{dashboard ? <TimestampText value={dashboard.generated_at} /> : '-'}</Text>
        </Space>
      </div>
      {error ? <Banner type="danger" description={`加载失败：${error}`} /> : null}
      {dashboard ? (
        <>
          <section className="admin-dashboard-kpi-grid">
            {kpis.map((card) => (
              <Card bordered={false} className="admin-dashboard-card" key={card.label} shadows="always">
                <Text type="secondary">{card.label}</Text>
                <Title heading={3}>{card.value}</Title>
                <Text type="tertiary">{card.description}</Text>
              </Card>
            ))}
          </section>
          <section className="admin-dashboard-detail-grid">
            <Card bordered={false} shadows="always">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>行情订阅</Title>
                <StatusTag value={dashboard.market.feed_runtime_status} />
                <Text>当前启动 providers：{joinList(dashboard.market.feed_providers)}</Text>
                <Text>运行 symbols：{joinList(dashboard.market.feed_symbols)}</Text>
                <Text>配置重载：{dashboard.market.feed_needs_reload ? '需要重载' : '无需重载'}</Text>
              </Space>
            </Card>
            <Card bordered={false} shadows="always">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>资金与链上状态</Title>
                <Banner type="warning" description={custodyText(dashboard.wallet.custody_status)} />
                <Text>待解禁：{displayNumber(dashboard.wallet.pending_unlocks)}</Text>
                <Text>待充值确认：{displayNumber(dashboard.wallet.pending_deposits)}</Text>
                <Text>待提现处理：{displayNumber(dashboard.wallet.pending_withdrawals)}</Text>
              </Space>
            </Card>
            <Card bordered={false} shadows="always">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>产品运行</Title>
                <Text>秒合约未结算订单：{displayNumber(dashboard.products.seconds_open_orders)}</Text>
                <Text>杠杆持仓：{displayNumber(dashboard.products.margin_open_positions)}</Text>
                <Text>24h 强平：{displayNumber(dashboard.products.margin_liquidated_24h)}</Text>
                <Text>Earn 生效申购：{displayNumber(dashboard.products.earn_active_subscriptions)}</Text>
                <Text>24h 到期 Earn：{displayNumber(dashboard.products.earn_maturing_24h)}</Text>
              </Space>
            </Card>
            <Card bordered={false} shadows="always">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>风控 / 事件积压</Title>
                <Text>24h 风控事件：{displayNumber(dashboard.risk.risk_events_24h)}</Text>
                <Text>24h 阻断事件：{displayNumber(dashboard.risk.blocked_events_24h)}</Text>
                <Text>Outbox 待发布：{displayNumber(dashboard.risk.pending_outbox_events)}</Text>
                <Text>Inbox 重试：{displayNumber(dashboard.risk.retry_inbox_events)}</Text>
                <Text>Inbox 死信：{displayNumber(dashboard.risk.dead_letter_inbox_events)}</Text>
              </Space>
            </Card>
            <Card bordered={false} className="admin-dashboard-audit-card" shadows="always">
              <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                <Title heading={4}>最新审计动作</Title>
                <Text type="secondary">24h 管理动作：{displayNumber(dashboard.audit.admin_actions_24h)}</Text>
                <div className="admin-dashboard-audit-list">
                  {dashboard.audit.latest_actions.length ? (
                    dashboard.audit.latest_actions.map((action) => (
                      <div className="admin-dashboard-audit-item" key={action.id}>
                        <div>
                          <Text strong>{action.action}</Text>
                          <Text type="tertiary">{action.target_type} #{action.target_id}</Text>
                        </div>
                        <TimestampText value={action.created_at} />
                      </div>
                    ))
                  ) : (
                    <Text type="secondary">暂无审计动作</Text>
                  )}
                </div>
              </Space>
            </Card>
          </section>
        </>
      ) : null}
    </main>
  );
}
