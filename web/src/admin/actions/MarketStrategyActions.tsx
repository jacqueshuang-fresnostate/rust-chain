import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Title } = Typography;

type CreateValues = {
  endTime: string;
  pairId: string;
  startPrice: string;
  startTime: string;
  status: string;
  strategyType: string;
  targetPrice: string;
  volatility: string;
  volumeMax: string;
  volumeMin: string;
};

type StatusValues = {
  strategyId: string;
  status: string;
};

const initialCreate: CreateValues = {
  pairId: '',
  strategyType: 'linear',
  startPrice: '',
  targetPrice: '',
  startTime: '',
  endTime: '',
  volatility: '0',
  volumeMin: '0',
  volumeMax: '0',
  status: 'draft'
};
const initialStatus: StatusValues = { strategyId: '', status: 'active' };

function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function requiredTimestamp(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为 Unix 毫秒时间戳`);
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

export function MarketStrategyActions() {
  const [createValues, setCreateValues] = useState(initialCreate);
  const [statusValues, setStatusValues] = useState(initialStatus);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="行情策略动作" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建策略</Title>
            <div className="admin-action-form">
              <label>交易对ID<input value={createValues.pairId} onChange={(event) => setCreateValues({ ...createValues, pairId: event.currentTarget.value })} /></label>
              <label>策略类型<input value={createValues.strategyType} onChange={(event) => setCreateValues({ ...createValues, strategyType: event.currentTarget.value })} /></label>
              <label>起始价<input value={createValues.startPrice} onChange={(event) => setCreateValues({ ...createValues, startPrice: event.currentTarget.value })} /></label>
              <label>目标价<input value={createValues.targetPrice} onChange={(event) => setCreateValues({ ...createValues, targetPrice: event.currentTarget.value })} /></label>
              <label>开始时间戳<input value={createValues.startTime} onChange={(event) => setCreateValues({ ...createValues, startTime: event.currentTarget.value })} /></label>
              <label>结束时间戳<input value={createValues.endTime} onChange={(event) => setCreateValues({ ...createValues, endTime: event.currentTarget.value })} /></label>
              <label>波动率<input value={createValues.volatility} onChange={(event) => setCreateValues({ ...createValues, volatility: event.currentTarget.value })} /></label>
              <label>最小成交量<input value={createValues.volumeMin} onChange={(event) => setCreateValues({ ...createValues, volumeMin: event.currentTarget.value })} /></label>
              <label>最大成交量<input value={createValues.volumeMax} onChange={(event) => setCreateValues({ ...createValues, volumeMax: event.currentTarget.value })} /></label>
              <label>
                初始状态
                <select value={createValues.status} onChange={(event) => setCreateValues({ ...createValues, status: event.currentTarget.value })}>
                  <option value="draft">draft</option>
                  <option value="active">active</option>
                  <option value="paused">paused</option>
                  <option value="disabled">disabled</option>
                </select>
              </label>
            </div>
            <ConfirmAction
              actionText="创建策略"
              title="确认创建行情策略"
              onConfirm={(reason) =>
                submitAction('创建行情策略', () =>
                  apiRequest('/admin/api/v1/market-strategies', {
                    method: 'POST',
                    body: JSON.stringify({
                      pair_id: requiredPositiveInteger(createValues.pairId, '交易对ID'),
                      strategy_type: createValues.strategyType.trim(),
                      start_price: createValues.startPrice.trim(),
                      target_price: createValues.targetPrice.trim(),
                      start_time: requiredTimestamp(createValues.startTime, '开始时间'),
                      end_time: requiredTimestamp(createValues.endTime, '结束时间'),
                      volatility: createValues.volatility.trim(),
                      volume_min: createValues.volumeMin.trim(),
                      volume_max: createValues.volumeMax.trim(),
                      status: createValues.status,
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
            <Title heading={4}>更新策略状态</Title>
            <div className="admin-action-form">
              <label>策略ID<input value={statusValues.strategyId} onChange={(event) => setStatusValues({ ...statusValues, strategyId: event.currentTarget.value })} /></label>
              <label>
                目标状态
                <select value={statusValues.status} onChange={(event) => setStatusValues({ ...statusValues, status: event.currentTarget.value })}>
                  <option value="draft">draft</option>
                  <option value="active">active</option>
                  <option value="paused">paused</option>
                  <option value="disabled">disabled</option>
                </select>
              </label>
            </div>
            <ConfirmAction
              actionText="更新策略状态"
              title="确认更新行情策略状态"
              onConfirm={(reason) =>
                submitAction('更新行情策略状态', () =>
                  apiRequest(`/admin/api/v1/market-strategies/${requiredPositiveInteger(statusValues.strategyId, '策略ID')}/status`, {
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
