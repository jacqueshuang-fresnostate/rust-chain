import { Card, Space, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminTextArea, AdminTextInput } from '../../../shared/SemiFormControls';
import { BooleanSelect, type CreateActionProps, FormModal, type RowActionHelpers, booleanFromSelect, completeCreate, optionalString, recordString, requiredString, submitAction } from './shared';

type RiskRuleValues = {
  ruleType: string;
  targetType: string;
  targetId: string;
  configJson: string;
  enabled: string;
};

const initialRiskRule: RiskRuleValues = {
  ruleType: '',
  targetType: '',
  targetId: '',
  configJson: '{}',
  enabled: 'true'
};

function isRiskRuleCreatable(values: RiskRuleValues): boolean {
  return Boolean(values.ruleType.trim() && values.targetType.trim() && values.configJson.trim());
}

export function RiskRuleRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const ruleId = recordString(record, 'id');
  const enabled = record.enabled === true;
  const nextEnabled = !enabled;
  const actionText = enabled ? '禁用' : '启用';

  return (
    <ConfirmAction
      actionText={actionText}
      disabled={!ruleId}
      title={`${actionText}风控规则`}
      onConfirm={async (reason) => {
        await submitAction(`${actionText}风控规则`, () =>
          apiRequest(`/admin/api/v1/risk/rules/${ruleId}/status`, {
            method: 'PATCH',
            body: JSON.stringify({ enabled: nextEnabled, reason })
          })
        );
        helpers.reload();
      }}
    />
  );
}

export function CreateRiskRuleAction({ onCreated }: CreateActionProps = {}) {
  const [riskRule, setRiskRule] = useState(initialRiskRule);

  return (
    <FormModal actionText="添加风控规则" size="wide" title="添加风控规则">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <label>规则类型<AdminTextInput ariaLabel="规则类型" value={riskRule.ruleType} onChange={(ruleType) => setRiskRule({ ...riskRule, ruleType })} /></label>
            <label>对象类型<AdminTextInput ariaLabel="对象类型" value={riskRule.targetType} onChange={(targetType) => setRiskRule({ ...riskRule, targetType })} /></label>
            <label>对象ID<AdminTextInput ariaLabel="对象ID" value={riskRule.targetId} onChange={(targetId) => setRiskRule({ ...riskRule, targetId })} /></label>
            <label>规则配置JSON<AdminTextArea ariaLabel="规则配置JSON" autosize value={riskRule.configJson} onChange={(configJson) => setRiskRule({ ...riskRule, configJson })} /></label>
            <label>启用<BooleanSelect label="启用" value={riskRule.enabled} onChange={(enabled) => setRiskRule({ ...riskRule, enabled })} /></label>
          </div>
          <ConfirmAction
            actionText="提交添加风控规则"
            disabled={!isRiskRuleCreatable(riskRule)}
            title="确认添加风控规则"
            onConfirm={async (reason) => {
              let configJson: unknown;
              try {
                configJson = JSON.parse(riskRule.configJson);
              } catch {
                Toast.error('规则配置JSON格式错误');
                return;
              }

              await submitAction('添加风控规则', () =>
                apiRequest('/admin/api/v1/risk/rules', {
                  method: 'POST',
                  body: JSON.stringify({
                    rule_type: requiredString(riskRule.ruleType, '规则类型'),
                    target_type: requiredString(riskRule.targetType, '对象类型'),
                    target_id: optionalString(riskRule.targetId),
                    config_json: configJson,
                    enabled: booleanFromSelect(riskRule.enabled),
                    reason
                  })
                })
              );
              completeCreate(close, onCreated, () => setRiskRule(initialRiskRule));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
