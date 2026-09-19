import { Banner, Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import {
  changeAgentPassword,
  createAgentInviteCode,
  getAgentCommissions,
  getAgentConvertStats,
  getAgentDashboard,
  getAgentInviteCodes,
  getAgentMe,
  getAgentSubAgents,
  getAgentTeamTree,
  getAgentUsers,
  updateAgentInviteCodeStatus,
  type AgentCommission,
  type AgentCommissionsResponse,
  type AgentConvertStats,
  type AgentDashboard,
  type AgentInviteCode,
  type AgentMe,
  type AgentSubAgent,
  type AgentTeamTreeNode,
  type AgentTeamTreeResponse,
  type AgentTeamUser
} from '../api/agent';
import { PageHeader } from '../layouts/PageHeader';
import { AmountText } from '../shared/AmountText';
import { DataTable } from '../shared/DataTable';
import { AdminPasswordInput, AdminTextInput } from '../shared/SemiFormControls';
import { StatusTag } from '../shared/StatusTag';
import { TimestampText } from '../shared/TimestampText';
import { formatAdminNumber } from '../shared/numberFormat';
import { addDecimalText, decimalFitsStorage, formatDecimalText } from '../shared/decimal';
import { parseSafeInteger } from '../shared/integer';
import { OnlineSupportWorkbench } from '../support/OnlineSupportWorkbench';

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
  return formatAdminNumber(value) ?? '-';
}

export function commissionPageTotals(rows: AgentCommission[]): Array<{ assetId: number; amount: string | null }> {
  const totals = new Map<number, string | null>();
  for (const row of rows) {
    if (parseSafeInteger(row.payout_asset_id, 1) === null) continue;
    const assetId = row.payout_asset_id as number;
    const previous = totals.has(assetId) ? totals.get(assetId)! : '0';
    totals.set(assetId, previous === null || !decimalFitsStorage(row.commission_amount)
      ? null : addDecimalText(previous, row.commission_amount));
  }
  return [...totals].sort(([a], [b]) => a - b).map(([assetId, amount]) => ({ assetId, amount }));
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

function AgentPasswordChangeCard() {
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);

  async function submit() {
    setSubmitting(true);
    try {
      await changeAgentPassword(currentPassword.trim(), newPassword.trim());
      setCurrentPassword('');
      setNewPassword('');
      Toast.success('密码已修改，请使用新密码重新登录');
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Card bordered={false} shadows="always">
      <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
        <Title heading={4}>修改登录密码</Title>
        <label>当前密码<AdminPasswordInput ariaLabel="当前密码" value={currentPassword} onChange={setCurrentPassword} /></label>
        <label>新密码<AdminPasswordInput ariaLabel="新密码" value={newPassword} onChange={setNewPassword} /></label>
        <Text type="tertiary">新密码需为 6-20 位，且与当前密码不同；修改后当前登录会话立即失效。</Text>
        <Button disabled={!currentPassword.trim() || !newPassword.trim()} loading={submitting} onClick={submit} theme="solid" type="primary">
          提交修改
        </Button>
      </Space>
    </Card>
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
        description: '按发放资产分别核对'
      },
      {
        label: '待结算佣金',
        value: '按资产查看',
        description: '不同资产金额不合计'
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
                <Title heading={4}>佣金资产汇总</Title>
                {data.dashboard.commission_assets?.map((row, index) => <div key={row.payout_asset_id ?? `unknown-${index}`}>
                  <Text>{row.payout_asset_id === null ? '发放资产未确认' : `资产 ID ${row.payout_asset_id}`}：</Text>
                  {row.payout_asset_id === null ? <Text>暂不合计</Text> : <Text>待结算 <AmountText value={row.pending_commission_amount} />；已结算 <AmountText value={row.settled_commission_amount} />；累计 <AmountText value={row.total_commission_amount} /></Text>}
                </div>) ?? <Text>暂缺按资产统计</Text>}
              </Space>
            </Card>
            <AgentPasswordChangeCard />
          </section>
        </>
      ) : loading ? <Text type="secondary">加载中</Text> : null}
    </main>
  );
}

export function AgentSupportPage() {
  return <OnlineSupportWorkbench scope="agent" />;
}

export function AgentUsersPage() {
  const navigate = useNavigate();
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
      { dataIndex: 'referred_at', key: 'referred_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '加入时间' },
      {
        dataIndex: 'user_id',
        fixed: 'right',
        key: 'actions',
        render: (_value, record) => (
          <Button
            onClick={() => navigate(`/agent/users/${record.user_id}/portfolio`, { state: { email: record.email ?? null } })}
            size="small"
            type="primary"
          >
            资产与订单
          </Button>
        ),
        title: '操作',
        width: 288
      }
    ],
    [navigate]
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
    const limit = trimmed ? parseSafeInteger(trimmed, 1, 2_147_483_647) : undefined;
    if (limit === null) {
      Toast.error('使用上限必须为 1 至 2147483647 的安全整数');
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
      <Banner description="最新启用邀请码会与代理关联用户的手机端邀请页同步。" type="info" />
      <DataTable columns={columns} data={data ?? []} error={error} loading={loading} />
    </main>
  );
}

export { AgentUserPortfolioPage } from './UserPortfolioPage';

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
      { dataIndex: 'payout_asset_id', key: 'payout_asset_id', title: '发放资产ID', render: (value) => value ?? '未确认' },
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
          <Space wrap>
            <Text>已加载记录数：{data.commissions.length}</Text>
            {commissionPageTotals(data.commissions).map(({ assetId, amount }) =>
              <Text key={assetId}>已加载佣金（资产 ID {assetId}）：{amount === null ? '数值无效' : formatDecimalText(amount, { minimumFractionDigits: 0, maximumFractionDigits: 18 })}</Text>
            )}
            {data.commissions.some((row) => row.payout_asset_id == null) ? <Text>发放资产未确认的记录不计入汇总</Text> : null}
            <Text type="tertiary">仅当前已加载记录，非全量；不同资产不合计</Text>
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
        { label: '金额统计', value: '暂缺按资产统计', description: '不同转出与转入资产的金额不合计' }
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

const subAgentColumns: Array<ColumnProps<AgentSubAgent>> = [
  { dataIndex: 'agent_code', key: 'agent_code', title: '代理编号' },
  { dataIndex: 'level', key: 'level', render: (value) => `L${String(value || 1)}`, title: '层级' },
  { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
  { dataIndex: 'direct_user_count', key: 'direct_user_count', title: '直属用户数' },
  { dataIndex: 'team_user_count', key: 'team_user_count', title: '团队用户数' },
  { dataIndex: 'parent_agent_id', key: 'parent_agent_id', title: '上级代理ID' }
];

export function AgentSubAgentsPage() {
  const loadSubAgents = useCallback(async () => (await getAgentSubAgents()).agents, []);
  const { data, error, loading } = useLoader<AgentSubAgent[]>(loadSubAgents);

  return (
    <main className="exchange-page">
      <PageHeader title="下级代理" />
      <DataTable columns={subAgentColumns} data={data ?? []} error={error} loading={loading} rowKey="id" />
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
      <Title heading={4}>下级代理</Title>
      <DataTable columns={subAgentColumns} data={data?.agents ?? []} error={error} loading={loading} rowKey="id" />
      <Title heading={4}>团队用户</Title>
      <DataTable columns={columns} data={data?.nodes ?? []} error={error} loading={loading} rowKey="user_id" />
    </main>
  );
}
