import { IconList, IconPlus, IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Descriptions, Popconfirm, SideSheet, Space, Tabs, Typography, Toast } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import type { ApiRecord } from '../../api/types';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { DataTable } from '../../shared/DataTable';
import { DetailDrawer, type DetailDrawerData } from '../../shared/DetailDrawer';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';
import { AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';

const { Text, Title } = Typography;

type AgentRecord = Record<string, unknown> & {
  admin_status?: string | null;
  admin_username?: string | null;
  agent_code?: string | null;
  created_at?: number | null;
  email?: string | null;
  id: number | string;
  level?: number | string | null;
  parent_agent_code?: string | null;
  parent_agent_id?: number | string | null;
  root_agent_code?: string | null;
  root_agent_id?: number | string | null;
  direct_user_count?: number | string | null;
  team_user_count?: number | string | null;
  child_agent_count?: number | string | null;
  status?: string | null;
  user_id?: number | string | null;
};

type AgentCreateValues = {
  adminPassword: string;
  adminUsername: string;
  agentCode: string;
  parentAgentId: string;
  userId: string;
};

const initialCreateValues: AgentCreateValues = {
  adminPassword: '',
  adminUsername: '',
  agentCode: '',
  parentAgentId: '',
  userId: ''
};

function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function requiredString(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label}不能为空`);
  }
  return trimmed;
}

function optionalParentAgentId(value: string): number | undefined {
  const trimmed = value.trim();
  return trimmed ? requiredPositiveInteger(trimmed, '上级代理') : undefined;
}

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function recordString(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
}

function agentStatusActions(status: string): Array<{ label: string; status: string }> {
  return [
    { label: '启用', status: 'active' },
    { label: '暂停', status: 'suspended' },
    { label: '禁用', status: 'disabled' }
  ].filter((item) => item.status !== status);
}

function isAgentCreatable(values: AgentCreateValues) {
  return Boolean(values.userId.trim() && values.agentCode.trim() && values.adminUsername.trim() && values.adminPassword.trim());
}

type AgentUserRecord = Record<string, unknown> & {
  user_id: number | string;
};

type AgentDetailDrawerProps = {
  agentId: string | null;
  agentOptions: Array<{ label: string; value: string }>;
  onClose: () => void;
  onReassigned: () => void;
};

function AgentDetailDrawer({ agentId, agentOptions, onClose, onReassigned }: AgentDetailDrawerProps) {
  const [agent, setAgent] = useState<AgentRecord | null>(null);
  const [users, setUsers] = useState<AgentUserRecord[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);
  const [targetAgents, setTargetAgents] = useState<Record<string, string>>({});
  const [reloadVersion, setReloadVersion] = useState(0);
  const reassignOptions = useMemo(() => agentOptions.filter((option) => option.value !== agentId), [agentId, agentOptions]);

  useEffect(() => {
    if (!agentId) {
      setAgent(null);
      setUsers([]);
      setError(null);
      setTargetAgents({});
      return undefined;
    }

    let active = true;
    setLoading(true);
    setError(null);

    Promise.all([
      apiRequest<AgentRecord>(`/admin/api/v1/agents/${agentId}`),
      apiRequest<{ users?: AgentUserRecord[] }>(`/admin/api/v1/agents/${agentId}/users`)
    ])
      .then(([agentResponse, usersResponse]) => {
        if (active) {
          setAgent(agentResponse);
          setUsers(Array.isArray(usersResponse.users) ? usersResponse.users : []);
        }
      })
      .catch((caught: unknown) => {
        if (!active) {
          return;
        }
        setAgent(null);
        setUsers([]);
        setError(caught instanceof Error ? caught : new Error('加载代理详情失败'));
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [agentId, reloadVersion]);

  async function reassignUser(userId: string) {
    await submitAction('转移用户归属', () =>
      apiRequest(`/admin/api/v1/users/${userId}/agent`, {
        method: 'PATCH',
        body: JSON.stringify({ agent_id: requiredPositiveInteger(targetAgents[userId] ?? '', '目标代理') })
      })
    );
    setTargetAgents((current) => {
      const next = { ...current };
      delete next[userId];
      return next;
    });
    setReloadVersion((value) => value + 1);
    onReassigned();
  }

  const agentInfo = agent
    ? [
        { key: '代理编号', value: recordString(agent, 'agent_code') },
        { key: '层级', value: `L${recordString(agent, 'level') || '1'}` },
        { key: '状态', value: <StatusTag value={typeof agent.status === 'string' ? agent.status : null} /> },
        { key: '邮箱', value: recordString(agent, 'email') || '-' },
        { key: '直属上级', value: recordString(agent, 'parent_agent_code') || '总代理' },
        { key: '归属总代理', value: recordString(agent, 'root_agent_code') || '-' },
        { key: '直属用户', value: recordString(agent, 'direct_user_count') || '0' },
        { key: '团队用户', value: recordString(agent, 'team_user_count') || '0' },
        { key: '下级代理', value: recordString(agent, 'child_agent_count') || '0' },
        { key: '代理后台账号', value: recordString(agent, 'admin_username') || '-' },
        { key: '后台账号状态', value: <StatusTag value={typeof agent.admin_status === 'string' ? agent.admin_status : null} /> },
        { key: '创建时间', value: <TimestampText value={typeof agent.created_at === 'number' ? agent.created_at : null} /> }
      ]
    : [];

  const userColumns: Array<ColumnProps<AgentUserRecord>> = [
    { dataIndex: 'user_id', key: 'user_id', title: '用户ID' },
    { dataIndex: 'email', key: 'email', title: '邮箱' },
    { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
    { dataIndex: 'kyc_level', key: 'kyc_level', title: 'KYC等级' },
    { dataIndex: 'owner_agent_code', key: 'owner_agent_code', title: '归属代理' },
    { dataIndex: 'depth', key: 'depth', title: '层级深度' },
    { dataIndex: 'referred_at', key: 'referred_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '加入时间' },
    {
      dataIndex: 'user_id',
      key: 'reassign',
      render: (_value, record) => {
        const userId = recordString(record, 'user_id');
        const target = targetAgents[userId] ?? '';
        return (
          <Space spacing={6}>
            <AdminSelect
              ariaLabel={`用户${userId}目标代理`}
              onChange={(value) => setTargetAgents((current) => ({ ...current, [userId]: value }))}
              optionList={reassignOptions}
              placeholder="目标代理"
              value={target}
            />
            <Popconfirm
              content="该用户将被重新分配至所选代理"
              okText="确认转移"
              onConfirm={() => reassignUser(userId)}
              title="确认转移用户归属"
            >
              <Button disabled={!userId || !target} size="small" type="danger">转移</Button>
            </Popconfirm>
          </Space>
        );
      },
      title: '转移归属',
      width: 280
    }
  ];

  return (
    <SideSheet onCancel={onClose} title="代理详情" visible={agentId !== null} width="min(920px, calc(100vw - 48px))">
      <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
        {agent ? <Descriptions align="plain" column={3} data={agentInfo} layout="horizontal" /> : null}
        <Title heading={5}>团队用户</Title>
        <DataTable columns={userColumns} data={users} error={error} loading={loading} rowKey="user_id" />
      </Space>
    </SideSheet>
  );
}

export function AgentManagementPage() {
  const [activeTab, setActiveTab] = useState<'list' | 'create'>('list');
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [createValues, setCreateValues] = useState(initialCreateValues);
  const [detail, setDetail] = useState<DetailDrawerData | null>(null);
  const [detailAgentId, setDetailAgentId] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);
  const [reloadVersion, setReloadVersion] = useState(0);
  const reload = useCallback(() => setReloadVersion((value) => value + 1), []);
  const parentAgentOptions = useMemo(
    () => [
      { label: '无上级（创建总代理）', value: '' },
      ...agents
        .filter((agent) => Number(agent.level || 1) < 3 && agent.status === 'active')
        .map((agent) => ({
          label: `${recordString(agent, 'agent_code')}（L${recordString(agent, 'level') || '1'}）`,
          value: recordString(agent, 'id')
        }))
    ],
    [agents]
  );
  const derivedLevel = useMemo(() => {
    const parent = agents.find((agent) => recordString(agent, 'id') === createValues.parentAgentId);
    return parent ? Number(parent.level || 1) + 1 : 1;
  }, [agents, createValues.parentAgentId]);
  const reassignAgentOptions = useMemo(
    () =>
      agents
        .filter((agent) => agent.status === 'active')
        .map((agent) => ({
          label: `${recordString(agent, 'agent_code')}（L${recordString(agent, 'level') || '1'}）`,
          value: recordString(agent, 'id')
        })),
    [agents]
  );

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);

    apiRequest<{ agents?: AgentRecord[] }>('/admin/api/v1/agents')
      .then((response) => {
        if (active) {
          setAgents(Array.isArray(response.agents) ? response.agents : []);
        }
      })
      .catch((caught: unknown) => {
        if (!active) {
          return;
        }
        setAgents([]);
        setError(caught instanceof Error ? caught : new Error('加载代理列表失败'));
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [reloadVersion]);

  async function openAgentDetail(agentId: string) {
    try {
      const agent = await apiRequest<AgentRecord>(`/admin/api/v1/agents/${agentId}`);
      setDetail({ title: '代理详情', data: agent as ApiRecord });
    } catch (caught) {
      Toast.error(errorMessage(caught));
      throw caught;
    }
  }

  async function updateAgentStatus(agentId: string, nextStatus: string, reason: string) {
    await submitAction('更新代理状态', () =>
      apiRequest(`/admin/api/v1/agents/${agentId}/status`, {
        method: 'PATCH',
        body: JSON.stringify({ status: nextStatus, reason })
      })
    );
    reload();
  }

  const columns = useMemo<Array<ColumnProps<AgentRecord>>>(
    () => [
      { dataIndex: 'id', key: 'id', title: '代理ID' },
      { dataIndex: 'user_id', key: 'user_id', title: '用户ID' },
      { dataIndex: 'email', key: 'email', title: '邮箱' },
      { dataIndex: 'agent_code', key: 'agent_code', title: '代理编号' },
      { dataIndex: 'level', key: 'level', render: (value) => `L${String(value || 1)}`, title: '层级' },
      { dataIndex: 'parent_agent_code', key: 'parent_agent_code', render: (value) => typeof value === 'string' && value ? value : '总代理', title: '直属上级' },
      { dataIndex: 'root_agent_code', key: 'root_agent_code', title: '归属总代理' },
      { dataIndex: 'direct_user_count', key: 'direct_user_count', title: '直属用户' },
      { dataIndex: 'child_agent_count', key: 'child_agent_count', title: '下级代理' },
      { dataIndex: 'team_user_count', key: 'team_user_count', title: '团队用户' },
      { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
      { dataIndex: 'admin_username', key: 'admin_username', title: '代理后台账号' },
      { dataIndex: 'admin_status', key: 'admin_status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '后台账号状态' },
      { dataIndex: 'created_at', key: 'created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '创建时间' },
      {
        dataIndex: 'id',
        key: 'actions',
        render: (_value, record) => {
          const agentId = recordString(record, 'id');
          const status = recordString(record, 'status');
          return (
            <Space spacing={6} wrap>
              <Button disabled={!agentId} onClick={() => setDetailAgentId(agentId)} size="small" theme="borderless">
                详情
              </Button>
              <Button disabled={!agentId} onClick={() => openAgentDetail(agentId)} size="small" theme="borderless">
                查看详情
              </Button>
              {agentStatusActions(status).map((action) => (
                <ConfirmAction
                  actionText={action.label}
                  disabled={!agentId}
                  key={action.status}
                  title={`${action.label}代理`}
                  onConfirm={(reason) => updateAgentStatus(agentId, action.status, reason)}
                />
              ))}
            </Space>
          );
        },
        title: '操作',
        width: 320
      }
    ],
    []
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={reload} theme="borderless">
            刷新
          </Button>
        }
        title="代理管理"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Tabs
          activeKey={activeTab}
          className="admin-action-tabs"
          onChange={(nextTab) => setActiveTab(nextTab as 'list' | 'create')}
          tabBarExtraContent={<Text type="tertiary">共 {agents.length} 个代理</Text>}
          tabList={[
            { itemKey: 'list', tab: '代理列表', icon: <IconList aria-hidden="true" /> },
            { itemKey: 'create', tab: '创建代理', icon: <IconPlus aria-hidden="true" /> }
          ]}
          type="button"
        />
        <div className="admin-action-workbench-grid">
          {activeTab === 'create' ? (
          <section className="admin-action-panel">
            <Title heading={4}>创建代理</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>用户ID<AdminTextInput ariaLabel="用户ID" value={createValues.userId} onChange={(userId) => setCreateValues({ ...createValues, userId })} /></label>
              <label>代理编号<AdminTextInput ariaLabel="代理编号" value={createValues.agentCode} onChange={(agentCode) => setCreateValues({ ...createValues, agentCode })} /></label>
              <label>代理后台账号<AdminTextInput ariaLabel="代理后台账号" value={createValues.adminUsername} onChange={(adminUsername) => setCreateValues({ ...createValues, adminUsername })} /></label>
              <label>初始密码<AdminPasswordInput ariaLabel="初始密码" value={createValues.adminPassword} onChange={(adminPassword) => setCreateValues({ ...createValues, adminPassword })} /></label>
              <label>直属上级<AdminSelect ariaLabel="直属上级" optionList={parentAgentOptions} value={createValues.parentAgentId} onChange={(parentAgentId) => setCreateValues({ ...createValues, parentAgentId })} /></label>
              <label>所属层级<AdminTextInput ariaLabel="所属层级" readOnly value={`L${derivedLevel}`} onChange={() => undefined} /></label>
            </div>
            <ConfirmAction
              actionText="创建代理"
              disabled={!isAgentCreatable(createValues)}
              title="确认创建代理"
              onConfirm={async (reason) => {
                await submitAction('创建代理', () =>
                  apiRequest('/admin/api/v1/agents', {
                    method: 'POST',
                    body: JSON.stringify({
                      user_id: requiredPositiveInteger(createValues.userId, '用户ID'),
                      agent_code: requiredString(createValues.agentCode, '代理编号'),
                      admin_username: requiredString(createValues.adminUsername, '代理后台账号'),
                      admin_password: requiredString(createValues.adminPassword, '初始密码'),
                      parent_agent_id: optionalParentAgentId(createValues.parentAgentId),
                      reason
                    })
                  })
                );
                setCreateValues(initialCreateValues);
                reload();
              }}
            />
          </section>
          ) : null}
          {activeTab === 'list' ? (
          <section className="admin-action-panel">
            <Title heading={4}>代理列表</Title>
            <DataTable columns={columns} data={agents} error={error} loading={loading} />
          </section>
          ) : null}
        </div>
      </Card>
      <DetailDrawer detail={detail} onClose={() => setDetail(null)} />
      <AgentDetailDrawer
        agentId={detailAgentId}
        agentOptions={reassignAgentOptions}
        onClose={() => setDetailAgentId(null)}
        onReassigned={reload}
      />
    </main>
  );
}
