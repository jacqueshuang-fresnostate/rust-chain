import { Banner, Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';

const { Title } = Typography;

type AgentCreateValues = {
  adminPasswordHash: string;
  adminUsername: string;
  agentCode: string;
  level: string;
  userId: string;
};

type AgentStatusValues = {
  agentId: string;
  status: string;
};

const initialCreateValues: AgentCreateValues = {
  adminPasswordHash: '',
  adminUsername: '',
  agentCode: '',
  level: '1',
  userId: ''
};

const initialStatusValues: AgentStatusValues = {
  agentId: '',
  status: 'active'
};

function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function optionalPositiveInteger(value: string): number | undefined {
  if (!value.trim()) {
    return undefined;
  }
  return requiredPositiveInteger(value, '层级');
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

export function AgentManagementPage() {
  const [createValues, setCreateValues] = useState(initialCreateValues);
  const [statusValues, setStatusValues] = useState(initialStatusValues);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="代理管理" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建代理</Title>
            <div className="admin-action-form">
              <label>
                用户ID
                <AdminTextInput ariaLabel="用户ID" value={createValues.userId} onChange={(userId) => setCreateValues({ ...createValues, userId })} />
              </label>
              <label>
                代理编号
                <AdminTextInput ariaLabel="代理编号" value={createValues.agentCode} onChange={(agentCode) => setCreateValues({ ...createValues, agentCode })} />
              </label>
              <label>
                代理后台账号
                <AdminTextInput ariaLabel="代理后台账号" value={createValues.adminUsername} onChange={(adminUsername) => setCreateValues({ ...createValues, adminUsername })} />
              </label>
              <label>
                密码哈希
                <AdminPasswordInput ariaLabel="密码哈希" value={createValues.adminPasswordHash} onChange={(adminPasswordHash) => setCreateValues({ ...createValues, adminPasswordHash })} />
              </label>
              <label>
                层级
                <AdminTextInput ariaLabel="层级" value={createValues.level} onChange={(level) => setCreateValues({ ...createValues, level })} />
              </label>
            </div>
            <Banner fullMode={false} type="warning" description="密码哈希由后端现有接口接收；请勿在前端填入明文密码。" />
            <ConfirmAction
              actionText="创建代理"
              title="确认创建代理"
              onConfirm={(reason) =>
                submitAction('创建代理', () =>
                  apiRequest('/admin/api/v1/agents', {
                    method: 'POST',
                    body: JSON.stringify({
                      user_id: requiredPositiveInteger(createValues.userId, '用户ID'),
                      agent_code: createValues.agentCode.trim(),
                      admin_username: createValues.adminUsername.trim(),
                      admin_password_hash: createValues.adminPasswordHash.trim(),
                      level: optionalPositiveInteger(createValues.level),
                      reason
                    })
                  })
                )
              }
            />
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>更新代理状态</Title>
            <div className="admin-action-form">
              <label>
                代理ID
                <AdminTextInput ariaLabel="代理ID" value={statusValues.agentId} onChange={(agentId) => setStatusValues({ ...statusValues, agentId })} />
              </label>
              <label>
                目标状态
                <AdminSelect
                  ariaLabel="目标状态"
                  onChange={(status) => setStatusValues({ ...statusValues, status })}
                  optionList={[
                    { value: 'active', label: 'active' },
                    { value: 'suspended', label: 'suspended' },
                    { value: 'disabled', label: 'disabled' }
                  ]}
                  value={statusValues.status}
                />
              </label>
            </div>
            <ConfirmAction
              actionText="更新状态"
              title="确认更新代理状态"
              onConfirm={(reason) =>
                submitAction('更新代理状态', () =>
                  apiRequest(`/admin/api/v1/agents/${requiredPositiveInteger(statusValues.agentId, '代理ID')}/status`, {
                    method: 'PATCH',
                    body: JSON.stringify({ status: statusValues.status, reason })
                  })
                )
              }
            />
          </Space>
        </Card>
      </div>
    </main>
  );
}
