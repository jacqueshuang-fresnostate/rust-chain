import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';

const { Title } = Typography;

type ProductValues = {
  productId: string;
  status: string;
};

const initialValues: ProductValues = { productId: '', status: 'active' };

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

export function ProductStatusActions() {
  const [values, setValues] = useState(initialValues);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="理财产品动作" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>更新理财产品状态</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>理财产品ID<AdminTextInput ariaLabel="理财产品ID" value={values.productId} onChange={(productId) => setValues({ ...values, productId })} /></label>
              <label>
                目标状态
                <AdminSelect
                  ariaLabel="目标状态"
                  onChange={(status) => setValues({ ...values, status })}
                  optionList={[
                    { value: 'active', label: '启用' },
                    { value: 'disabled', label: '禁用' }
                  ]}
                  value={values.status}
                />
              </label>
            </div>
            <ConfirmAction
              actionText="更新理财产品状态"
              title="确认更新理财产品状态"
              onConfirm={(reason) =>
                submitAction('更新理财产品状态', () =>
                  apiRequest(`/admin/api/v1/earn/products/${requiredPositiveInteger(values.productId, '理财产品ID')}/status`, {
                    method: 'PATCH',
                    body: JSON.stringify({ status: values.status, reason })
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
