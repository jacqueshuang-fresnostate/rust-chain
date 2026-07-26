import { Button, Card, SideSheet, Space, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminModalTriggerButton, AdminPasswordInput, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  AssetSelect,
  AssetStatusSelect,
  type CreateActionProps,
  type RowActionHelpers,
  createModalProps,
  errorMessage,
  isNonNegativeIntegerInput,
  openRecordDetail,
  optionalString,
  recordString,
  requiredNonNegativeInteger,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  useAssetOptions
} from './shared';

type UserValues = {
  email: string;
  phone: string;
  password: string;
  status: string;
  kycLevel: string;
};

type UserRechargeValues = {
  assetId: string;
  amount: string;
};

type AssignAgentValues = {
  agentId: string;
};

const initialUser: UserValues = {
  email: '',
  phone: '',
  password: '',
  status: 'active',
  kycLevel: '0'
};

const initialUserRecharge: UserRechargeValues = {
  assetId: '',
  amount: ''
};

const initialAssignAgent: AssignAgentValues = {
  agentId: ''
};

function isUserCreatable(values: UserValues): boolean {
  return Boolean((values.email.trim() || values.phone.trim()) && values.password.trim() && values.status.trim() && isNonNegativeIntegerInput(values.kycLevel));
}

function isUserRechargeSubmittable(values: UserRechargeValues): boolean {
  return Boolean(values.assetId.trim() && values.amount.trim() && Number(values.amount) > 0);
}

function isAssignAgentSubmittable(values: AssignAgentValues): boolean {
  return Boolean(values.agentId.trim() && Number(values.agentId) > 0);
}

async function openUserAssets(userId: string, helpers: RowActionHelpers) {
  try {
    const result = await apiRequest<ApiRecord>(`/admin/api/v1/wallet/accounts?user_id=${userId}&include_empty=true&limit=100`);
    const accounts = Array.isArray(result.accounts) ? (result.accounts as ApiRecord[]) : [];
    helpers.openDetail({ title: '用户资产', data: accounts });
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function UserRechargeAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  const [recharge, setRecharge] = useState(initialUserRecharge);
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <>
      <Button disabled={!userId} onClick={() => setVisible(true)} size="small" theme="borderless">
        充值
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="用户充值" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <AssetSelect label="充值资产" loading={assetLoading} options={assetOptions} value={recharge.assetId} onChange={(assetId) => setRecharge({ ...recharge, assetId })} />
              <label>充值金额<AdminTextInput ariaLabel="充值金额" value={recharge.amount} onChange={(amount) => setRecharge({ ...recharge, amount })} /></label>
            </div>
            <ConfirmAction
              actionText="提交充值"
              disabled={!isUserRechargeSubmittable(recharge)}
              title="确认用户充值"
              onConfirm={async (reason) => {
                await submitAction('用户充值', () =>
                  apiRequest(`/admin/api/v1/users/${userId}/recharge`, {
                    method: 'POST',
                    body: JSON.stringify({
                      asset_id: requiredPositiveInteger(recharge.assetId, '充值资产'),
                      amount: requiredString(recharge.amount, '充值金额'),
                      reason
                    })
                  })
                );
                setVisible(false);
                setRecharge(initialUserRecharge);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function AssignAgentAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  const [assignment, setAssignment] = useState(initialAssignAgent);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!userId} onClick={() => setVisible(true)} size="small" theme="borderless">
        分配代理
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="分配代理" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>用户ID<AdminTextInput ariaLabel="用户ID" readOnly value={userId} onChange={() => undefined} /></label>
              <label>代理ID<AdminTextInput ariaLabel="代理ID" value={assignment.agentId} onChange={(agentId) => setAssignment({ ...assignment, agentId })} /></label>
            </div>
            <ConfirmAction
              actionText="提交分配代理"
              disabled={!isAssignAgentSubmittable(assignment)}
              title="确认分配代理"
              onConfirm={async (reason) => {
                await submitAction('分配代理', () =>
                  apiRequest(`/admin/api/v1/users/${userId}/agent`, {
                    method: 'PATCH',
                    body: JSON.stringify({ agent_id: requiredPositiveInteger(assignment.agentId, '代理ID'), reason })
                  })
                );
                setVisible(false);
                setAssignment(initialAssignAgent);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function userStatusActions(status: string): Array<{ label: string; status: string }> {
  return [
    { label: '启用', status: 'active' },
    { label: '暂停', status: 'suspended' },
    { label: '封禁', status: 'disabled' }
  ].filter((item) => item.status !== status);
}

function UserStatusActions({ helpers, status, userId }: { helpers: RowActionHelpers; status: string; userId: string }) {
  return (
    <>
      {userStatusActions(status).map((action) => (
        <ConfirmAction
          actionText={action.label}
          disabled={!userId}
          key={action.status}
          title={`${action.label}用户`}
          onConfirm={async (reason) => {
            await submitAction(`${action.label}用户`, () =>
              apiRequest(`/admin/api/v1/users/${userId}/status`, {
                method: 'PATCH',
                body: JSON.stringify({ status: action.status, reason })
              })
            );
            helpers.reload();
          }}
        />
      ))}
    </>
  );
}

function ResetUserTwoFactorAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  return (
    <ConfirmAction
      actionText="重置2FA"
      disabled={!userId}
      title="重置用户2FA"
      onConfirm={async (reason) => {
        await submitAction('重置用户2FA', () =>
          apiRequest(`/admin/api/v1/users/${userId}/2fa/reset`, {
            method: 'POST',
            body: JSON.stringify({ reason })
          })
        );
        helpers.reload();
      }}
    />
  );
}

export function UserRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const userId = recordString(record, 'id');

  return (
    <>
      <Button disabled={!userId} onClick={() => openRecordDetail('/admin/api/v1/users', userId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <Button disabled={!userId} onClick={() => openUserAssets(userId, helpers)} size="small" theme="borderless">
        查看资产
      </Button>
      <UserRechargeAction helpers={helpers} userId={userId} />
      <AssignAgentAction helpers={helpers} userId={userId} />
      <ResetUserTwoFactorAction helpers={helpers} userId={userId} />
      <UserStatusActions helpers={helpers} status={recordString(record, 'status')} userId={userId} />
    </>
  );
}

export function CreateUserAction({ onCreated }: CreateActionProps = {}) {
  const [user, setUser] = useState(initialUser);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加用户</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加用户" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>邮箱<AdminTextInput ariaLabel="邮箱" value={user.email} onChange={(email) => setUser({ ...user, email })} /></label>
              <label>手机号<AdminTextInput ariaLabel="手机号" value={user.phone} onChange={(phone) => setUser({ ...user, phone })} /></label>
              <label>登录密码<AdminPasswordInput ariaLabel="登录密码" value={user.password} onChange={(password) => setUser({ ...user, password })} /></label>
              <label>状态<AssetStatusSelect value={user.status} onChange={(status) => setUser({ ...user, status })} /></label>
              <label>KYC等级<AdminTextInput ariaLabel="KYC等级" value={user.kycLevel} onChange={(kycLevel) => setUser({ ...user, kycLevel })} /></label>
            </div>
            <ConfirmAction
              actionText="提交添加用户"
              disabled={!isUserCreatable(user)}
              title="确认添加用户"
              onConfirm={async (reason) => {
                await submitAction('添加用户', () =>
                  apiRequest('/admin/api/v1/users', {
                    method: 'POST',
                    body: JSON.stringify({
                      email: optionalString(user.email),
                      phone: optionalString(user.phone),
                      password: requiredString(user.password, '登录密码'),
                      status: requiredString(user.status, '状态'),
                      kyc_level: requiredNonNegativeInteger(user.kycLevel, 'KYC等级'),
                      reason
                    })
                  })
                );
                setVisible(false);
                setUser(initialUser);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}
