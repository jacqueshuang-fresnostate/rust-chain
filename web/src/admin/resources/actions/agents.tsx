import { Button, Card, SideSheet, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import { type RowActionHelpers, createModalProps, recordString, requiredPositiveInteger, requiredString, statusOptions, submitAction } from './shared';

type AgentCommissionRuleValues = {
  agentId: string;
  commissionRate: string;
  productType: string;
  status: string;
};

const initialAgentCommissionRule: AgentCommissionRuleValues = {
  agentId: '',
  commissionRate: '',
  productType: 'convert',
  status: 'active'
};

function isAgentCommissionRuleSubmittable(values: AgentCommissionRuleValues, includeAgentId: boolean): boolean {
  return Boolean((!includeAgentId || values.agentId.trim()) && values.productType.trim() && values.commissionRate.trim() && values.status.trim());
}

const agentCommissionRuleProductOptions: SemiSelectOption[] = [
  { value: 'convert', label: '闪兑' },
  { value: 'prediction', label: '竞猜' },
  { value: 'spot', label: '现货' },
  { value: 'margin', label: '杠杆' },
  { value: 'seconds_contract', label: '秒合约' }
];

function AgentCommissionRuleForm({ includeAgentId, onChange, values }: { includeAgentId: boolean; onChange: (values: AgentCommissionRuleValues) => void; values: AgentCommissionRuleValues }) {
  return (
    <div className="admin-action-form">
      {includeAgentId ? <label>代理ID<AdminTextInput ariaLabel="代理ID" value={values.agentId} onChange={(agentId) => onChange({ ...values, agentId })} /></label> : null}
      {!includeAgentId ? <label>代理ID<AdminTextInput ariaLabel="代理ID" readOnly value={values.agentId} onChange={() => undefined} /></label> : null}
      <label>
        产品类型
        <AdminSelect ariaLabel="产品类型" disabled={!includeAgentId} onChange={(productType) => onChange({ ...values, productType })} optionList={agentCommissionRuleProductOptions} value={values.productType} />
      </label>
      <label>佣金比例<AdminTextInput ariaLabel="佣金比例" value={values.commissionRate} onChange={(commissionRate) => onChange({ ...values, commissionRate })} /></label>
      <label>
        {includeAgentId ? '初始状态' : '状态'}
        <AdminSelect ariaLabel={includeAgentId ? '初始状态' : '状态'} onChange={(status) => onChange({ ...values, status })} optionList={statusOptions} value={values.status} />
      </label>
    </div>
  );
}

export function CreateAgentCommissionRuleAction({ onCreated }: { onCreated?: () => void }) {
  const [rule, setRule] = useState(initialAgentCommissionRule);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加佣金规则</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加佣金规则" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <AgentCommissionRuleForm includeAgentId values={rule} onChange={setRule} />
            <ConfirmAction
              actionText="提交添加佣金规则"
              disabled={!isAgentCommissionRuleSubmittable(rule, true)}
              title="确认添加佣金规则"
              onConfirm={async (reason) => {
                await submitAction('添加佣金规则', () =>
                  apiRequest('/admin/api/v1/agent-commission-rules', {
                    method: 'POST',
                    body: JSON.stringify({
                      agent_id: requiredPositiveInteger(rule.agentId, '代理ID'),
                      product_type: rule.productType,
                      commission_rate: requiredString(rule.commissionRate, '佣金比例'),
                      status: rule.status,
                      reason
                    })
                  })
                );
                setVisible(false);
                setRule(initialAgentCommissionRule);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function AgentCommissionRuleRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const ruleId = recordString(record, 'id');
  const [rule, setRule] = useState<AgentCommissionRuleValues>({
    agentId: recordString(record, 'agent_id'),
    productType: recordString(record, 'product_type') || 'convert',
    commissionRate: recordString(record, 'commission_rate'),
    status: recordString(record, 'status') || 'active'
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!ruleId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改佣金规则" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <AgentCommissionRuleForm includeAgentId={false} values={rule} onChange={setRule} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isAgentCommissionRuleSubmittable(rule, false)}
              title="确认修改佣金规则"
              onConfirm={async (reason) => {
                await submitAction('修改佣金规则', () =>
                  apiRequest(`/admin/api/v1/agent-commission-rules/${ruleId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      commission_rate: requiredString(rule.commissionRate, '佣金比例'),
                      status: rule.status,
                      reason
                    })
                  })
                );
                setVisible(false);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function AgentCommissionRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const commissionId = recordString(record, 'id');
  const canUpdate = recordString(record, 'status') === 'pending';

  async function updateStatus(status: 'settled' | 'rejected', reason: string) {
    await submitAction(status === 'settled' ? '结算代理佣金' : '拒绝代理佣金', () =>
      apiRequest(`/admin/api/v1/agent-commissions/${commissionId}/status`, {
        method: 'PATCH',
        body: JSON.stringify({ status, reason })
      })
    );
    helpers.reload();
  }

  return (
    <>
      <ConfirmAction actionText="结算" disabled={!commissionId || !canUpdate} title="结算代理佣金" onConfirm={(reason) => updateStatus('settled', reason)} />
      <ConfirmAction actionText="拒绝" disabled={!commissionId || !canUpdate} title="拒绝代理佣金" onConfirm={(reason) => updateStatus('rejected', reason)} />
    </>
  );
}
