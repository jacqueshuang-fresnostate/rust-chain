import { Banner, Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useMemo, useState } from 'react';

import {
  createAgentInviteCode,
  getAgentCommissions,
  getAgentConvertStats,
  getAgentDashboard,
  getAgentInviteCodes,
  getAgentMe,
  getAgentTeamTree,
  getAgentUsers,
  updateAgentInviteCodeStatus,
  type AgentCommission,
  type AgentCommissionsResponse,
  type AgentConvertStats,
  type AgentDashboard,
  type AgentInviteCode,
  type AgentMe,
  type AgentTeamTreeNode,
  type AgentTeamTreeResponse,
  type AgentTeamUser
} from '../api/agent';
import { PageHeader } from '../layouts/PageHeader';
import { AmountText } from '../shared/AmountText';
import { DataTable } from '../shared/DataTable';
import { AdminTextInput } from '../shared/SemiFormControls';
import { StatusTag } from '../shared/StatusTag';
import { TimestampText } from '../shared/TimestampText';
import { formatAdminNumber } from '../shared/numberFormat';

const { Text, Title } = Typography;

type LoadState<T> = {
  data: T | null;
  error: Error | null;
  loading: boolean;
};

type KpiCard = {
  label: string;
  value: string;
  description: string;
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : '加载失败';
}

function displayNumber(value: string | number | null | undefined) {
  return formatAdminNumber(value) ?? '0';
}

function useLoader<T>(load: () => Promise<T>, initialReload = 0): LoadState<T> {
  const [state, setState] = useState<LoadState<T>>({ data: null, error: null, loading: true });

  useEffect(() => {
    let active = true;
    setState((current) => ({ ...current, error: null, loading: true }));

    load()
      .then((data) => {
        if (active) {
          setState({ data, error: null, loading: false });
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setState({ data: null, error: error instanceof Error ? error : new Error(errorMessage(error)), loading: false });
        }
      });

    return () => {
      active = false;
    };
  }, [load, initialReload]);

  return state;
}

function ErrorBanner({ error }: { error?: Error | null }) {
  return error ? <Banner type="danger" description={`加载失败：${error.message}`} /> : null;
}

function KpiGrid({ cards }: { cards: KpiCard[] }) {
  return (
    <section className="admin-dashboard-kpi-grid">
      {cards.map((card) => (
        <Card bordered={false} className="admin-dashboard-card" key={card.label} shadows="always">
          <Text type="secondary">{card.label}</Text>
          <Title heading={3}>{card.value}</Title>
          <Text type="tertiary">{card.description}</Text>
        </Card>
      ))}
    </section>
  );
}

export function AgentDashboardPage() {
  const loadDashboard = useCallback(
    async () => {
      const [me, dashboard, convertStats] = await Promise.all([getAgentMe(), getAgentDashboard(), getAgentConvertStats()]);
      return { me, dashboard, convertStats };
    },
    []
  );
  const { data, error, loading } = useLoader<{ me: AgentMe; dashboard: AgentDashboard; convertStats: AgentConvertStats }>(loadDashboard);

  const kpis = useMemo<KpiCard[]>(() => {
    if (!data) {
      return [];
    }

    return [
      {
        label: '团队人数',
        value: displayNumber(data.dashboard.team_user_count),
        description: `活跃邀请码 ${displayNumber(data.dashboard.active_invite_code_count)}`
      },
      {
        label: '佣金记录',
        value: displayNumber(data.dashboard.commission_record_count),
        description: `总佣金 ${displayNumber(data.dashboard.total_commission_amount)}`
      },
      {
        label: '待结算佣金',
        value: displayNumber(data.dashboard.pending_commission_amount),
        description: `已结算 ${displayNumber(data.dashboard.settled_commission_amount)}`
      },
      {
        label: '闪兑订单',
        value: displayNumber(data.convertStats.total_orders),
        description: `待处理 ${displayNumber(data.convertStats.pending_orders)}，已完成 ${displayNumber(data.convertStats.completed_orders)}`
      }
    ];
  }, [data]);

  return (
    <main className="exchange-page admin-dashboard-page">
      <PageHeader title="代理总览" />
      <ErrorBanner error={error} />
      {data ? (
        <>
          <Card bordered={false} shadows="always" style={{ marginBottom: 16 }}>
            <Space align="start" spacing={12} vertical>
              <Title heading={4}>{data.me.username}</Title>
              <Text>代理编号：{data.me.agent_code}</Text>
              <Text>代理ID：{data.me.agent_id}</Text>
              <Text>层级：{data.me.level}</Text>
              <Space>
                <Text>代理状态</Text>
                <StatusTag value={data.me.agent_status} />
                <Text>账号状态</Text>
                <StatusTag value={data.me.admin_status} />
              </Space>
              <Text>最近登录：{<TimestampText value={data.me.last_login_at ?? null} />}</Text>
            </Space>
          </Card>
          <KpiGrid cards={kpis} />
          <section className="admin-dashboard-detail-grid">
            <Card bordered={false} shadows="always">
              <Space align="start" spacing={12} vertical>
                <Title heading={4}>闪兑累计</Title>
                <Text>转出金额：<AmountText value={data.convertStats.total_from_amount} /></Text>
                <Text>转入金额：<AmountText value={data.convertStats.total_to_amount} /></Text>
              </Space>
            </Card>
          </section>
        </>
      ) : loading ? <Text type="secondary">加载中</Text> : null}
    </main>
  );
}

export function AgentUsersPage() {
  const loadUsers = useCallback(async () => (await getAgentUsers()).users, []);
  const { data, error, loading } = useLoader<AgentTeamUser[]>(loadUsers);
  const columns = useMemo<Array<ColumnProps<AgentTeamUser>>>(
    () => [
      { dataIndex: 'user_id', key: 'user_id', title: '用户ID' },
      { dataIndex: 'email', key: 'email', title: '邮箱' },
      { dataIndex: 'phone', key: 'phone', title: '手机号' },
      { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
      { dataIndex: 'kyc_level', key: 'kyc_level', title: 'KYC等级' },
      { dataIndex: 'depth', key: 'depth', title: '层级深度' },
      { dataIndex: 'referred_at', key: 'referred_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '加入时间' }
    ],
    []
  );

  return (
    <main className="exchange-page">
      <PageHeader title="团队用户" />
      <DataTable columns={columns} data={data ?? []} error={error} loading={loading} rowKey="user_id" />
    </main>
  );
}

export function AgentInviteCodesPage() {
  const [usageLimit, setUsageLimit] = useState('');
  const [reloadVersion, setReloadVersion] = useState(0);
  const reload = useCallback(() => setReloadVersion((value) => value + 1), []);
  const loadInviteCodes = useCallback(async () => (await getAgentInviteCodes()).invite_codes, [reloadVersion]);
  const { data, error, loading } = useLoader<AgentInviteCode[]>(loadInviteCodes, reloadVersion);

  async function createInviteCode() {
    const trimmed = usageLimit.trim();
    const limit = trimmed ? Number(trimmed) : undefined;
    if (trimmed && (!Number.isInteger(limit) || Number(limit) <= 0)) {
      Toast.error('使用上限必须为正整数');
      return;
    }

    try {
      await createAgentInviteCode(limit);
      Toast.success('邀请码已创建');
      setUsageLimit('');
      reload();
    } catch (caught) {
      Toast.error(errorMessage(caught));
    }
  }

  async function updateStatus(inviteCodeId: number, nextStatus: 'active' | 'disabled') {
    try {
      await updateAgentInviteCodeStatus(inviteCodeId, nextStatus);
      Toast.success('邀请码状态已更新');
      reload();
    } catch (caught) {
      Toast.error(errorMessage(caught));
    }
  }

  const columns = useMemo<Array<ColumnProps<AgentInviteCode>>>(
    () => [
      { dataIndex: 'id', key: 'id', title: 'ID' },
      { dataIndex: 'code', key: 'code', title: '邀请码' },
      { dataIndex: 'usage_limit', key: 'usage_limit', title: '使用上限' },
      { dataIndex: 'used_count', key: 'used_count', title: '已使用' },
      { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
      { dataIndex: 'created_at', key: 'created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '创建时间' },
      {
        dataIndex: 'id',
        key: 'actions',
        render: (_value, record) => {
          const nextStatus = record.status === 'active' ? 'disabled' : 'active';
          return (
            <Button onClick={() => updateStatus(record.id, nextStatus)} size="small" type={nextStatus === 'disabled' ? 'danger' : 'primary'}>
              {nextStatus === 'disabled' ? '禁用' : '启用'}
            </Button>
          );
        },
        title: '操作',
        width: 120
      }
    ],
    []
  );

  return (
    <main className="exchange-page">
      <PageHeader
        actions={
          <Space>
            <AdminTextInput ariaLabel="使用上限" onChange={setUsageLimit} placeholder="使用上限" value={usageLimit} />
            <Button onClick={createInviteCode} theme="solid" type="primary">创建邀请码</Button>
          </Space>
        }
        title="邀请码"
      />
      <DataTable columns={columns} data={data ?? []} error={error} loading={loading} />
    </main>
  );
}

export function AgentCommissionsPage() {
  const { data, error, loading } = useLoader<AgentCommissionsResponse>(getAgentCommissions);
  const columns = useMemo<Array<ColumnProps<AgentCommission>>>(
    () => [
      { dataIndex: 'id', key: 'id', title: 'ID' },
      { dataIndex: 'user_id', key: 'user_id', title: '用户ID' },
      { dataIndex: 'email', key: 'email', title: '邮箱' },
      { dataIndex: 'source_type', key: 'source_type', title: '来源类型' },
      { dataIndex: 'source_id', key: 'source_id', title: '来源ID' },
      { dataIndex: 'source_amount', key: 'source_amount', render: (value) => <AmountText value={typeof value === 'string' || typeof value === 'number' ? value : null} />, title: '来源金额' },
      { dataIndex: 'commission_amount', key: 'commission_amount', render: (value) => <AmountText value={typeof value === 'string' || typeof value === 'number' ? value : null} />, title: '佣金金额' },
      { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
      { dataIndex: 'depth', key: 'depth', title: '层级深度' },
      { dataIndex: 'payout_ledger_id', key: 'payout_ledger_id', title: '结算流水ID' },
      { dataIndex: 'payout_amount', key: 'payout_amount', render: (value) => <AmountText value={typeof value === 'string' || typeof value === 'number' ? value : null} />, title: '结算金额' },
      { dataIndex: 'payout_created_at', key: 'payout_created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '结算时间' },
      { dataIndex: 'created_at', key: 'created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '创建时间' }
    ],
    []
  );

  return (
    <main className="exchange-page">
      <PageHeader title="佣金记录" />
      {data ? (
        <Card bordered={false} shadows="always" style={{ marginBottom: 16 }}>
          <Space>
            <Text>记录数：{displayNumber(data.total_records)}</Text>
            <Text>总佣金：<AmountText value={data.total_commission_amount} /></Text>
          </Space>
        </Card>
      ) : null}
      <DataTable columns={columns} data={data?.commissions ?? []} error={error} loading={loading} />
    </main>
  );
}

export function AgentConvertStatsPage() {
  const { data, error, loading } = useLoader<AgentConvertStats>(getAgentConvertStats);
  const cards = data
    ? [
        { label: '总订单', value: displayNumber(data.total_orders), description: `代理ID ${data.agent_id}` },
        { label: '待处理订单', value: displayNumber(data.pending_orders), description: '当前仍待处理的闪兑订单' },
        { label: '已完成订单', value: displayNumber(data.completed_orders), description: '已完成的闪兑订单' },
        { label: '累计转出', value: displayNumber(data.total_from_amount), description: `累计转入 ${displayNumber(data.total_to_amount)}` }
      ]
    : [];

  return (
    <main className="exchange-page admin-dashboard-page">
      <PageHeader title="闪兑统计" />
      <ErrorBanner error={error} />
      {loading ? <Text type="secondary">加载中</Text> : <KpiGrid cards={cards} />}
    </main>
  );
}

export function AgentTeamTreePage() {
  const loadTeamTree = useCallback(async () => await getAgentTeamTree(), []);
  const { data, error, loading } = useLoader<AgentTeamTreeResponse>(loadTeamTree);
  const columns = useMemo<Array<ColumnProps<AgentTeamTreeNode>>>(
    () => [
      { dataIndex: 'user_id', key: 'user_id', title: '用户ID' },
      { dataIndex: 'email', key: 'email', title: '邮箱' },
      { dataIndex: 'phone', key: 'phone', title: '手机号' },
      { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
      { dataIndex: 'direct_inviter_id', key: 'direct_inviter_id', title: '直接邀请人ID' },
      { dataIndex: 'direct_inviter_type', key: 'direct_inviter_type', title: '直接邀请人类型' },
      { dataIndex: 'depth', key: 'depth', title: '层级深度' },
      { dataIndex: 'path', key: 'path', title: '路径' },
      { dataIndex: 'referred_at', key: 'referred_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '加入时间' }
    ],
    []
  );

  return (
    <main className="exchange-page">
      <PageHeader title="团队树" />
      {data ? <Text type="secondary">根代理ID：{data.root_agent_id}</Text> : null}
      <DataTable columns={columns} data={data?.nodes ?? []} error={error} loading={loading} rowKey="user_id" />
    </main>
  );
}
