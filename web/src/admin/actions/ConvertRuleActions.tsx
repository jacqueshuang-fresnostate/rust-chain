import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Title } = Typography;

type RuleValues = {
  convertPairId: string;
  fixedRate: string;
  status: string;
};

const initialRule: RuleValues = { convertPairId: '', fixedRate: '', status: 'active' };

function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
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

export function ConvertRuleActions() {
  const [values, setValues] = useState(initialRule);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="新币闪兑规则" />
      <Card bordered={false} shadows="always">
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>新增或更新固定汇率</Title>
          <div className="admin-action-form admin-action-form-narrow">
            <label>闪兑交易对ID<input value={values.convertPairId} onChange={(event) => setValues({ ...values, convertPairId: event.currentTarget.value })} /></label>
            <label>固定汇率<input value={values.fixedRate} onChange={(event) => setValues({ ...values, fixedRate: event.currentTarget.value })} /></label>
            <label>
              状态
              <select value={values.status} onChange={(event) => setValues({ ...values, status: event.currentTarget.value })}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
          </div>
          <ConfirmAction
            actionText="提交规则"
            title="确认提交新币闪兑规则"
            onConfirm={(reason) =>
              submitAction('提交新币闪兑规则', () =>
                apiRequest('/admin/api/v1/convert/new-coin-rules', {
                  method: 'POST',
                  body: JSON.stringify({
                    convert_pair_id: requiredPositiveInteger(values.convertPairId, '闪兑交易对ID'),
                    rate_source: 'fixed',
                    fixed_rate: values.fixedRate.trim(),
                    floating_rate_json: undefined,
                    status: values.status,
                    reason
                  })
                })
              )
            }
          />
        </Space>
      </Card>
    </main>
  );
}
