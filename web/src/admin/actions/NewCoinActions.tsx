import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Text, Title } = Typography;

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

export function NewCoinActions() {
  const [lifecycle, setLifecycle] = useState(initialLifecycle);
  const [distribute, setDistribute] = useState(initialDistribute);
  const [unlockRule, setUnlockRule] = useState(initialUnlockRule);
  const [unlockFee, setUnlockFee] = useState(initialUnlockFee);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="新币生命周期动作" description="覆盖生命周期流转、后台派发、解禁规则和矿工费规则更新。" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>生命周期流转</Title>
            <Text type="secondary">按后端顺序推进 preheat → subscription → distribution → listed。</Text>
            <div className="admin-action-form">
              <label>项目ID<input value={lifecycle.projectId} onChange={(event) => setLifecycle({ ...lifecycle, projectId: event.currentTarget.value })} /></label>
              <label>
                目标阶段
                <select value={lifecycle.lifecycleStatus} onChange={(event) => setLifecycle({ ...lifecycle, lifecycleStatus: event.currentTarget.value })}>
                  <option value="preheat">preheat</option>
                  <option value="subscription">subscription</option>
                  <option value="distribution">distribution</option>
                  <option value="listed">listed</option>
                </select>
              </label>
              <label>上市时间戳<input placeholder="listed 可选，Unix ms" value={lifecycle.listedAt} onChange={(event) => setLifecycle({ ...lifecycle, listedAt: event.currentTarget.value })} /></label>
            </div>
            <ConfirmAction
              actionText="更新生命周期"
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
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>后台派发</Title>
            <Text type="secondary">项目必须处于 distribution 阶段，幂等键用于避免重复派发。</Text>
            <div className="admin-action-form">
              <label>项目ID<input value={distribute.projectId} onChange={(event) => setDistribute({ ...distribute, projectId: event.currentTarget.value })} /></label>
              <label>用户ID<input value={distribute.userId} onChange={(event) => setDistribute({ ...distribute, userId: event.currentTarget.value })} /></label>
              <label>申购ID<input value={distribute.subscriptionId} onChange={(event) => setDistribute({ ...distribute, subscriptionId: event.currentTarget.value })} /></label>
              <label>派发数量<input value={distribute.quantity} onChange={(event) => setDistribute({ ...distribute, quantity: event.currentTarget.value })} /></label>
              <label>幂等键<input value={distribute.idempotencyKey} onChange={(event) => setDistribute({ ...distribute, idempotencyKey: event.currentTarget.value })} /></label>
            </div>
            <ConfirmAction
              actionText="执行派发"
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
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>解禁规则</Title>
            <Text type="secondary">时间字段按 Unix milliseconds 输入，relative_period 使用秒数。</Text>
            <div className="admin-action-form">
              <label>项目ID<input value={unlockRule.projectId} onChange={(event) => setUnlockRule({ ...unlockRule, projectId: event.currentTarget.value })} /></label>
              <label>
                解禁类型
                <select value={unlockRule.unlockType} onChange={(event) => setUnlockRule({ ...unlockRule, unlockType: event.currentTarget.value })}>
                  <option value="immediate_on_listing">immediate_on_listing</option>
                  <option value="fixed_time">fixed_time</option>
                  <option value="relative_period">relative_period</option>
                </select>
              </label>
              <label>上市时间戳<input value={unlockRule.listedAt} onChange={(event) => setUnlockRule({ ...unlockRule, listedAt: event.currentTarget.value })} /></label>
              <label>固定解禁时间戳<input value={unlockRule.fixedUnlockAt} onChange={(event) => setUnlockRule({ ...unlockRule, fixedUnlockAt: event.currentTarget.value })} /></label>
              <label>相对解禁秒数<input value={unlockRule.relativeUnlockSeconds} onChange={(event) => setUnlockRule({ ...unlockRule, relativeUnlockSeconds: event.currentTarget.value })} /></label>
            </div>
            <ConfirmAction
              actionText="更新解禁规则"
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
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>矿工费规则</Title>
            <Text type="secondary">启用矿工费时需提供费率、计费依据和费用资产。</Text>
            <div className="admin-action-form">
              <label>项目ID<input value={unlockFee.projectId} onChange={(event) => setUnlockFee({ ...unlockFee, projectId: event.currentTarget.value })} /></label>
              <label className="admin-action-checkbox"><input checked={unlockFee.feeEnabled} type="checkbox" onChange={(event) => setUnlockFee({ ...unlockFee, feeEnabled: event.currentTarget.checked })} /> 启用矿工费</label>
              <label>费率<input value={unlockFee.feeRate} onChange={(event) => setUnlockFee({ ...unlockFee, feeRate: event.currentTarget.value })} /></label>
              <label>
                计费依据
                <select value={unlockFee.feeBasis} onChange={(event) => setUnlockFee({ ...unlockFee, feeBasis: event.currentTarget.value })}>
                  <option value="market_value">market_value</option>
                  <option value="profit">profit</option>
                </select>
              </label>
              <label>费用资产ID<input value={unlockFee.feeAsset} onChange={(event) => setUnlockFee({ ...unlockFee, feeAsset: event.currentTarget.value })} /></label>
            </div>
            <ConfirmAction
              actionText="更新矿工费"
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
          </Space>
        </Card>
      </div>
    </main>
  );
}
