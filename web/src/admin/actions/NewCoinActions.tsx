import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { AdminRequestActionBoundary } from '../access';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminCheckbox, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import {
  AdminReferenceSelect,
  type AdminReferenceOption,
  isReferenceSelectable,
  useAdminReferenceOptions
} from '../referenceOptions';

const { Title } = Typography;

type LifecycleValues = {
  projectId: string;
  lifecycleStatus: string;
  listedAt: string;
};

type DistributeValues = {
  idempotencyKey: string;
  projectId: string;
  quantity: string;
  subscriptionId: string;
  userId: string;
};

type UnlockRuleValues = {
  fixedUnlockAt: string;
  listedAt: string;
  projectId: string;
  relativeUnlockSeconds: string;
  unlockType: string;
};

type UnlockFeeValues = {
  feeAsset: string;
  feeBasis: string;
  feeEnabled: boolean;
  feeRate: string;
  projectId: string;
};

const initialLifecycle: LifecycleValues = { projectId: '', lifecycleStatus: 'subscription', listedAt: '' };
const initialDistribute: DistributeValues = { projectId: '', userId: '', subscriptionId: '', quantity: '', idempotencyKey: '' };
const initialUnlockRule: UnlockRuleValues = {
  projectId: '',
  unlockType: 'immediate_on_listing',
  listedAt: '',
  fixedUnlockAt: '',
  relativeUnlockSeconds: ''
};
const initialUnlockFee: UnlockFeeValues = { projectId: '', feeEnabled: false, feeRate: '', feeBasis: 'market_value', feeAsset: '' };

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
  return requiredPositiveInteger(value, '可选ID');
}

function optionalTimestamp(value: string): number | undefined {
  if (!value.trim()) {
    return undefined;
  }
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error('时间必须为 Unix 毫秒时间戳');
  }
  return parsed;
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

function projectIdFromLocation(): string {
  if (typeof window === 'undefined') {
    return '';
  }
  const hashQuery = window.location.hash.includes('?') ? window.location.hash.split('?').slice(1).join('?') : '';
  return new URLSearchParams(hashQuery || window.location.search).get('project_id')?.trim() ?? '';
}

function distributionProjectOptions(options: AdminReferenceOption[]): AdminReferenceOption[] {
  return options.map((option) => {
    if (option.disabled || option.lifecycleStatus === 'distribution') {
      return option;
    }
    return {
      ...option,
      disabled: true,
      disabledReason: '项目必须处于派发中才能执行后台派发'
    };
  });
}

export function NewCoinActions() {
  const [lifecycle, setLifecycle] = useState(initialLifecycle);
  const [distribute, setDistribute] = useState(initialDistribute);
  const [unlockRule, setUnlockRule] = useState(initialUnlockRule);
  const [unlockFee, setUnlockFee] = useState(initialUnlockFee);
  const projectReferences = useAdminReferenceOptions('newCoinProject');
  const userReferences = useAdminReferenceOptions('user');
  const assetReferences = useAdminReferenceOptions('asset');
  const distributableProjects = useMemo(() => distributionProjectOptions(projectReferences.options), [projectReferences.options]);
  const linkedProjectId = useMemo(projectIdFromLocation, []);

  useEffect(() => {
    if (!linkedProjectId || !projectReferences.options.some((option) => option.value === linkedProjectId)) {
      return;
    }
    setLifecycle((current) => (current.projectId ? current : { ...current, projectId: linkedProjectId }));
    setDistribute((current) => (current.projectId ? current : { ...current, projectId: linkedProjectId }));
    setUnlockRule((current) => (current.projectId ? current : { ...current, projectId: linkedProjectId }));
    setUnlockFee((current) => (current.projectId ? current : { ...current, projectId: linkedProjectId }));
  }, [linkedProjectId, projectReferences.options]);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="新币生命周期动作" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>生命周期流转</Title>
            <div className="admin-action-form">
              <AdminReferenceSelect
                error={projectReferences.error}
                label="新币项目"
                loading={projectReferences.loading}
                onChange={(projectId) => setLifecycle({ ...lifecycle, projectId })}
                options={projectReferences.options}
                placeholder="搜索项目符号或 ID"
                value={lifecycle.projectId}
              />
              <label>
                目标阶段
                <AdminSelect
                  ariaLabel="目标阶段"
                  onChange={(lifecycleStatus) => setLifecycle({ ...lifecycle, lifecycleStatus })}
                  optionList={[
                    { value: 'preheat', label: '预热' },
                    { value: 'subscription', label: '申购中' },
                    { value: 'distribution', label: '分发中' },
                    { value: 'listed', label: '已上市' }
                  ]}
                  value={lifecycle.lifecycleStatus}
                />
              </label>
              <label>上市时间戳<AdminTextInput ariaLabel="上市时间戳" placeholder="listed 可选，Unix ms" value={lifecycle.listedAt} onChange={(listedAt) => setLifecycle({ ...lifecycle, listedAt })} /></label>
            </div>
            <AdminRequestActionBoundary endpoint={`/admin/api/v1/new-coins/${lifecycle.projectId}/lifecycle`} method="PATCH">
              <ConfirmAction
              actionText="更新生命周期"
              disabled={!isReferenceSelectable(projectReferences.options, lifecycle.projectId)}
              title="确认更新新币生命周期"
              onConfirm={(reason) =>
                submitAction('更新生命周期', () =>
                  apiRequest(`/admin/api/v1/new-coins/${requiredPositiveInteger(lifecycle.projectId, '项目ID')}/lifecycle`, {
                    method: 'PATCH',
                    body: JSON.stringify({ lifecycle_status: lifecycle.lifecycleStatus, listed_at: optionalTimestamp(lifecycle.listedAt), reason })
                  })
                )
              }
              />
            </AdminRequestActionBoundary>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>后台派发</Title>
            <div className="admin-action-form">
              <AdminReferenceSelect
                error={projectReferences.error}
                label="派发项目"
                loading={projectReferences.loading}
                onChange={(projectId) => setDistribute({ ...distribute, projectId })}
                options={distributableProjects}
                placeholder="搜索处于派发中的项目"
                value={distribute.projectId}
              />
              <AdminReferenceSelect
                error={userReferences.error}
                label="接收用户"
                loading={userReferences.loading}
                onChange={(userId) => setDistribute({ ...distribute, userId })}
                options={userReferences.options}
                placeholder="搜索用户邮箱、手机或 ID"
                value={distribute.userId}
              />
              <label>申购ID<AdminTextInput ariaLabel="申购ID" value={distribute.subscriptionId} onChange={(subscriptionId) => setDistribute({ ...distribute, subscriptionId })} /></label>
              <label>派发数量<AdminTextInput ariaLabel="派发数量" value={distribute.quantity} onChange={(quantity) => setDistribute({ ...distribute, quantity })} /></label>
              <label>幂等键<AdminTextInput ariaLabel="幂等键" value={distribute.idempotencyKey} onChange={(idempotencyKey) => setDistribute({ ...distribute, idempotencyKey })} /></label>
            </div>
            <AdminRequestActionBoundary endpoint={`/admin/api/v1/new-coins/${distribute.projectId}/distribute`} method="POST">
              <ConfirmAction
              actionText="执行派发"
              disabled={
                !isReferenceSelectable(distributableProjects, distribute.projectId) ||
                !isReferenceSelectable(userReferences.options, distribute.userId)
              }
              title="确认执行新币派发"
              onConfirm={(reason) =>
                submitAction('执行派发', () =>
                  apiRequest(`/admin/api/v1/new-coins/${requiredPositiveInteger(distribute.projectId, '项目ID')}/distribute`, {
                    method: 'POST',
                    body: JSON.stringify({
                      user_id: requiredPositiveInteger(distribute.userId, '用户ID'),
                      subscription_id: optionalPositiveInteger(distribute.subscriptionId),
                      quantity: distribute.quantity.trim(),
                      idempotency_key: distribute.idempotencyKey.trim(),
                      reason
                    })
                  })
                )
              }
              />
            </AdminRequestActionBoundary>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>解禁规则</Title>
            <div className="admin-action-form">
              <AdminReferenceSelect
                error={projectReferences.error}
                label="新币项目"
                loading={projectReferences.loading}
                onChange={(projectId) => setUnlockRule({ ...unlockRule, projectId })}
                options={projectReferences.options}
                placeholder="搜索项目符号或 ID"
                value={unlockRule.projectId}
              />
              <label>
                解禁类型
                <AdminSelect
                  ariaLabel="解禁类型"
                  onChange={(unlockType) => setUnlockRule({ ...unlockRule, unlockType })}
                  optionList={[
                    { value: 'immediate_on_listing', label: '上市即解禁' },
                    { value: 'fixed_time', label: '固定时间解禁' },
                    { value: 'relative_period', label: '相对周期解禁' }
                  ]}
                  value={unlockRule.unlockType}
                />
              </label>
              <label>上市时间戳<AdminTextInput ariaLabel="上市时间戳" value={unlockRule.listedAt} onChange={(listedAt) => setUnlockRule({ ...unlockRule, listedAt })} /></label>
              <label>固定解禁时间戳<AdminTextInput ariaLabel="固定解禁时间戳" value={unlockRule.fixedUnlockAt} onChange={(fixedUnlockAt) => setUnlockRule({ ...unlockRule, fixedUnlockAt })} /></label>
              <label>相对解禁秒数<AdminTextInput ariaLabel="相对解禁秒数" value={unlockRule.relativeUnlockSeconds} onChange={(relativeUnlockSeconds) => setUnlockRule({ ...unlockRule, relativeUnlockSeconds })} /></label>
            </div>
            <AdminRequestActionBoundary endpoint={`/admin/api/v1/new-coins/${unlockRule.projectId}/unlock-rule`} method="PATCH">
              <ConfirmAction
              actionText="更新解禁规则"
              disabled={!isReferenceSelectable(projectReferences.options, unlockRule.projectId)}
              title="确认更新解禁规则"
              onConfirm={(reason) =>
                submitAction('更新解禁规则', () =>
                  apiRequest(`/admin/api/v1/new-coins/${requiredPositiveInteger(unlockRule.projectId, '项目ID')}/unlock-rule`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      unlock_type: unlockRule.unlockType,
                      listed_at: optionalTimestamp(unlockRule.listedAt),
                      fixed_unlock_at: optionalTimestamp(unlockRule.fixedUnlockAt),
                      relative_unlock_seconds: optionalPositiveInteger(unlockRule.relativeUnlockSeconds),
                      reason
                    })
                  })
                )
              }
              />
            </AdminRequestActionBoundary>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>矿工费规则</Title>
            <div className="admin-action-form">
              <AdminReferenceSelect
                error={projectReferences.error}
                label="新币项目"
                loading={projectReferences.loading}
                onChange={(projectId) => setUnlockFee({ ...unlockFee, projectId })}
                options={projectReferences.options}
                placeholder="搜索项目符号或 ID"
                value={unlockFee.projectId}
              />
              <label className="admin-action-checkbox"><AdminCheckbox checked={unlockFee.feeEnabled} onChange={(feeEnabled) => setUnlockFee({ ...unlockFee, feeEnabled })}>启用矿工费</AdminCheckbox></label>
              <label>费率<AdminTextInput ariaLabel="费率" value={unlockFee.feeRate} onChange={(feeRate) => setUnlockFee({ ...unlockFee, feeRate })} /></label>
              <label>
                计费依据
                <AdminSelect
                  ariaLabel="计费依据"
                  onChange={(feeBasis) => setUnlockFee({ ...unlockFee, feeBasis })}
                  optionList={[
                    { value: 'market_value', label: '市值' },
                    { value: 'profit', label: '收益' }
                  ]}
                  value={unlockFee.feeBasis}
                />
              </label>
              {unlockFee.feeEnabled ? (
                <AdminReferenceSelect
                  error={assetReferences.error}
                  label="费用资产"
                  loading={assetReferences.loading}
                  onChange={(feeAsset) => setUnlockFee({ ...unlockFee, feeAsset })}
                  options={assetReferences.options}
                  placeholder="搜索资产符号或 ID"
                  value={unlockFee.feeAsset}
                />
              ) : null}
            </div>
            <AdminRequestActionBoundary endpoint={`/admin/api/v1/new-coins/${unlockFee.projectId}/unlock-fee-rule`} method="PATCH">
              <ConfirmAction
              actionText="更新矿工费"
              disabled={
                !isReferenceSelectable(projectReferences.options, unlockFee.projectId) ||
                (unlockFee.feeEnabled && !isReferenceSelectable(assetReferences.options, unlockFee.feeAsset))
              }
              title="确认更新矿工费规则"
              onConfirm={(reason) =>
                submitAction('更新矿工费', () =>
                  apiRequest(`/admin/api/v1/new-coins/${requiredPositiveInteger(unlockFee.projectId, '项目ID')}/unlock-fee-rule`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      unlock_fee_enabled: unlockFee.feeEnabled,
                      unlock_fee_rate: unlockFee.feeEnabled ? unlockFee.feeRate.trim() : undefined,
                      unlock_fee_basis: unlockFee.feeEnabled ? unlockFee.feeBasis : undefined,
                      unlock_fee_asset: unlockFee.feeEnabled ? optionalPositiveInteger(unlockFee.feeAsset) : undefined,
                      reason
                    })
                  })
                )
              }
              />
            </AdminRequestActionBoundary>
          </Space>
        </Card>
      </div>
    </main>
  );
}
