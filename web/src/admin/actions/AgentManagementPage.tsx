import { IconList, IconPlus, IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Space, Tabs, Typography, Toast } from '@douyinfe/semi-ui';
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

function recordString(record: AgentRecord, key: keyof AgentRecord): string {
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

export function AgentManagementPage() {
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [createValues, setCreateValues] = useState(initialCreateValues);
  const [detail, setDetail] = useState<DetailDrawerData | null>(null);
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
        width: 260
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
          className="admin-action-tabs"
          defaultActiveKey="list"
          tabBarExtraContent={<Text type="tertiary">共 {agents.length} 个代理</Text>}
          tabList={[
            { itemKey: 'list', tab: '代理列表', icon: <IconList aria-hidden="true" /> },
            { itemKey: 'create', tab: '创建代理', icon: <IconPlus aria-hidden="true" /> }
          ]}
          type="button"
        />
        <div className="admin-action-workbench-grid">
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
          <section className="admin-action-panel">
            <Title heading={4}>代理列表</Title>
            <DataTable columns={columns} data={agents} error={error} loading={loading} />
          </section>
        </div>
      </Card>
      <DetailDrawer detail={detail} onClose={() => setDetail(null)} />
    </main>
  );
}
