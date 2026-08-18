import { Banner, Button, Card, Space, Tag, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';
import './DashboardPage.css';

const { Text, Title } = Typography;

type DashboardResponse = {
  audit: {
    admin_actions_24h: number;
    latest_actions: DashboardAuditAction[];
  };
  environment: 'development' | 'production' | 'staging' | 'test';
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

type DashboardAuditAction = {
  action: string;
  admin_id: number;
  created_at: number;
  id: number;
  target_id: string;
  target_type: string;
};

type KpiCard = {
  description: string;
  label: string;
  tone: 'brand' | 'info' | 'neutral' | 'warning';
  value: string;
};

const ENVIRONMENT_META = {
  production: { color: 'red', label: '生产环境' },
  staging: { color: 'orange', label: '预发布环境' },
  test: { color: 'light-blue', label: '测试环境' },
  development: { color: 'grey', label: '开发环境' }
} as const;

const AUDIT_TARGET_LABELS: Record<string, string> = {
  admin_config_change_request: '高风险配置变更申请',
  admin_news_item: '公告',
  agent: '代理',
  agent_admin_user: '代理管理员',
  agent_commission: '代理佣金',
  agent_commission_rule: '代理佣金规则',
  asset: '资产',
  convert_pair: '闪兑交易对',
  country_config: '国家配置',
  deposit_address_pool: '充值地址池',
  deposit_network_config: '充值网络',
  earn_category: '理财分类',
  earn_product: '理财产品',
  event_outbox: '事件队列',
  kyc_config: 'KYC 规则',
  loan_product: '贷款产品',
  margin_product: '杠杆产品',
  market_feed_config: '行情订阅配置',
  market_source_credential: '行情源凭据',
  market_strategy: '行情策略',
  new_coin_convert_rule: '新币兑换规则',
  new_coin_distribution: '新币派发',
  new_coin_project: '新币项目',
  platform_brand_config: '平台品牌',
  prediction_asset_config: '竞猜资产配置',
  prediction_settings: '竞猜全局设置',
  quick_recharge_config: '快速充值配置',
  quick_recharge_order: '快速充值订单',
  risk_rule: '风控规则',
  seconds_contract_order: '秒合约订单',
  seconds_contract_product: '秒合约产品',
  security_policy: '安全策略',
  smtp_config: 'SMTP 配置',
  smtp_delivery_settings: 'SMTP 发信策略',
  spot_order: '现货订单',
  trading_pair: '交易对',
  upload_storage_config: '上传存储配置',
  user: '用户',
  user_kyc_submission: 'KYC 申请',
  user_referral: '用户代理关系',
  user_two_factor: '用户两步验证',
  wallet_account: '钱包账户'
};

const AUDIT_ACTION_LABELS: Record<string, string> = {
  'agent_admin_user.password.reset': '重置代理管理员密码',
  'config_change.applied': '应用高风险配置变更',
  'config_change.approved': '通过高风险配置变更',
  'config_change.rejected': '驳回高风险配置变更',
  'config_change.requested': '提交高风险配置变更',
  'event_outbox.requeue': '重排失败事件',
  'kyc.config.update': '更新 KYC 规则',
  'kyc.submission.approve': '通过 KYC 申请',
  'kyc.submission.reject': '驳回 KYC 申请',
  'market_strategy.kline_recovery.execute': '执行行情 K 线补偿',
  'seconds_contract_order.settle': '人工结算秒合约订单',
  'spot_order.cancel': '取消现货订单',
  'user_2fa.reset': '重置用户两步验证',
  'user_referral.assign_agent': '分配用户代理',
  'wallet.recharge': '人工充值用户钱包'
};

function errorMessage(error: unknown) {
  const message = error instanceof ApiError || error instanceof Error ? error.message : '加载失败';
  return message.split(/\r?\n/, 1)[0]?.trim() || '加载失败';
}

function joinList(values: string[]) {
  return values.length ? values.join(', ') : '-';
}

function custodyText(status: string) {
  if (status === 'not_configured') {
    return '链上托管未接入运行监听';
  }
  return status;
}

function displayNumber(value: number) {
  return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 0 }).format(value);
}

function auditTargetLabel(targetType: string) {
  return AUDIT_TARGET_LABELS[targetType] ?? '其他后台对象';
}

function auditActionLabel(action: DashboardAuditAction) {
  const exactLabel = AUDIT_ACTION_LABELS[action.action];
  if (exactLabel) {
    return exactLabel;
  }

  const target = auditTargetLabel(action.target_type);
  if (action.action.endsWith('.status.update') || action.action.endsWith('.update_status')) {
    return `变更${target}状态`;
  }
  if (action.action.endsWith('.config.update') || action.action.endsWith('.update')) {
    return `更新${target}`;
  }
  if (action.action.endsWith('.create')) {
    return `创建${target}`;
  }
  if (action.action.endsWith('.delete')) {
    return `删除${target}`;
  }
  if (action.action.endsWith('.save') || action.action.endsWith('.upsert')) {
    return `保存${target}`;
  }
  if (action.action.endsWith('.test')) {
    return `测试${target}`;
  }
  if (action.action.endsWith('.reload')) {
    return `重载${target}`;
  }
  if (action.action.endsWith('.reclaim')) {
    return `回收${target}`;
  }
  if (action.action.endsWith('.approve')) {
    return `通过${target}`;
  }
  if (action.action.endsWith('.reject')) {
    return `驳回${target}`;
  }
  return `处理${target}`;
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
      { label: '用户总数', value: displayNumber(dashboard.users.total), description: `活跃 ${displayNumber(dashboard.users.active)}，24h 新增 ${displayNumber(dashboard.users.new_24h)}`, tone: 'brand' },
      { label: '活跃资产', value: displayNumber(dashboard.wallet.active_assets), description: `钱包账户 ${displayNumber(dashboard.wallet.wallet_accounts)}，非零账户 ${displayNumber(dashboard.wallet.non_zero_accounts)}`, tone: 'info' },
      { label: '活跃交易对', value: displayNumber(dashboard.market.active_pairs), description: `外部行情 ${displayNumber(dashboard.market.external_pairs)}，策略行情 ${displayNumber(dashboard.market.strategy_pairs)}`, tone: 'info' },
      { label: '现货挂单', value: displayNumber(dashboard.trading.spot_open_orders), description: `24h 成交 ${displayNumber(dashboard.trading.spot_trades_24h)}`, tone: 'neutral' },
      { label: '24h 成交', value: displayNumber(dashboard.trading.spot_trades_24h), description: `闪兑待处理 ${displayNumber(dashboard.trading.convert_pending_orders)}`, tone: 'neutral' },
      {
        label: '事件积压',
        value: displayNumber(dashboard.risk.pending_outbox_events + dashboard.risk.retry_inbox_events + dashboard.risk.dead_letter_inbox_events),
        description: `风控事件 ${displayNumber(dashboard.risk.risk_events_24h)}，阻断 ${displayNumber(dashboard.risk.blocked_events_24h)}`,
        tone: 'warning'
      }
    ];
  }, [dashboard]);

  async function loadDashboard() {
    setLoading(true);
    setError(null);
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
      <PageHeader
        actions={
          <Space className="admin-dashboard-toolbar" wrap>
            {dashboard ? (
              <Tag
                className="admin-dashboard-environment"
                color={ENVIRONMENT_META[dashboard.environment]?.color ?? 'grey'}
                data-environment={dashboard.environment}
                data-semantic-color={ENVIRONMENT_META[dashboard.environment]?.color ?? 'grey'}
                size="large"
              >
                {ENVIRONMENT_META[dashboard.environment]?.label ?? '未知环境'}
              </Tag>
            ) : null}
            <Text type="secondary">数据时间：{dashboard ? <TimestampText value={dashboard.generated_at} /> : '-'}</Text>
            <Button loading={loading} onClick={loadDashboard} theme="solid" type="primary">
              刷新总览
            </Button>
          </Space>
        }
        description="集中查看用户、资金、交易、审计与风险事件的当前运行状态。"
        title="总览仪表盘"
      />
      {error ? (
        <div role="alert">
          <Banner type="danger" description={`加载失败：${error}`} />
        </div>
      ) : null}
      {loading && !dashboard ? (
        <section aria-live="polite" className="admin-table-state">
          正在加载总览数据…
        </section>
      ) : null}
      {!loading && !error && !dashboard ? (
        <section aria-live="polite" className="admin-table-state">
          暂无总览数据
        </section>
      ) : null}
      {dashboard ? (
        <>
          <section aria-label="核心运营指标" className="admin-dashboard-kpi-grid">
            {kpis.map((card) => (
              <Card bordered={false} className="admin-dashboard-card" data-tone={card.tone} key={card.label}>
                <div className="admin-dashboard-card-heading">
                  <Text type="secondary">{card.label}</Text>
                  <span aria-hidden="true" />
                </div>
                <Title heading={3}>{card.value}</Title>
                <Text type="tertiary">{card.description}</Text>
              </Card>
            ))}
          </section>
          <section className="admin-dashboard-detail-grid">
            <Card bordered={false} className="admin-dashboard-detail-card">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>行情订阅</Title>
                <StatusTag value={dashboard.market.feed_runtime_status} />
                <Text>当前启动 providers：{joinList(dashboard.market.feed_providers)}</Text>
                <Text>运行 symbols：{joinList(dashboard.market.feed_symbols)}</Text>
                <Text>配置重载：{dashboard.market.feed_needs_reload ? '需要重载' : '无需重载'}</Text>
              </Space>
            </Card>
            <Card bordered={false} className="admin-dashboard-detail-card admin-dashboard-detail-card-warning">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>资金与链上状态</Title>
                <Banner type="warning" description={custodyText(dashboard.wallet.custody_status)} />
                <Text>待解禁：{displayNumber(dashboard.wallet.pending_unlocks)}</Text>
                <Text>待充值确认：{displayNumber(dashboard.wallet.pending_deposits)}</Text>
                <Text>待提现处理：{displayNumber(dashboard.wallet.pending_withdrawals)}</Text>
              </Space>
            </Card>
            <Card bordered={false} className="admin-dashboard-detail-card">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>产品运行</Title>
                <Text>秒合约未结算订单：{displayNumber(dashboard.products.seconds_open_orders)}</Text>
                <Text>杠杆持仓：{displayNumber(dashboard.products.margin_open_positions)}</Text>
                <Text>24h 强平：{displayNumber(dashboard.products.margin_liquidated_24h)}</Text>
                <Text>Earn 生效申购：{displayNumber(dashboard.products.earn_active_subscriptions)}</Text>
                <Text>24h 到期 Earn：{displayNumber(dashboard.products.earn_maturing_24h)}</Text>
              </Space>
            </Card>
            <Card bordered={false} className="admin-dashboard-detail-card admin-dashboard-detail-card-risk">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>风控 / 事件积压</Title>
                <Text>24h 风控事件：{displayNumber(dashboard.risk.risk_events_24h)}</Text>
                <Text>24h 阻断事件：{displayNumber(dashboard.risk.blocked_events_24h)}</Text>
                <Text>Outbox 待发布：{displayNumber(dashboard.risk.pending_outbox_events)}</Text>
                <Text>Inbox 重试：{displayNumber(dashboard.risk.retry_inbox_events)}</Text>
                <Text>Inbox 死信：{displayNumber(dashboard.risk.dead_letter_inbox_events)}</Text>
              </Space>
            </Card>
            <Card bordered={false} className="admin-dashboard-detail-card admin-dashboard-audit-card">
              <div className="admin-dashboard-audit-content">
                <div className="admin-dashboard-audit-heading">
                  <div>
                    <Title heading={4}>最近配置与运营动作</Title>
                    <Text type="secondary">
                      24 小时管理操作：{displayNumber(dashboard.audit.admin_actions_24h)}
                    </Text>
                  </div>
                  <a className="admin-dashboard-audit-link" href="/admin/audit-logs">
                    查看全部审计日志
                  </a>
                </div>
                {dashboard.audit.latest_actions.length ? (
                  <ol aria-label="最近配置与运营动作" className="admin-dashboard-audit-list">
                    {dashboard.audit.latest_actions.map((action) => (
                      <li className="admin-dashboard-audit-item" key={action.id}>
                        <div className="admin-dashboard-audit-copy">
                          <Text strong>{auditActionLabel(action)}</Text>
                          <Text type="tertiary">
                            目标：{auditTargetLabel(action.target_type)} #{action.target_id || '-'}
                          </Text>
                        </div>
                        <TimestampText value={action.created_at} />
                      </li>
                    ))}
                  </ol>
                ) : (
                  <div aria-live="polite" className="admin-dashboard-audit-empty">
                    暂无最近配置或运营动作
                  </div>
                )}
              </div>
            </Card>
          </section>
        </>
      ) : null}
    </main>
  );
}
