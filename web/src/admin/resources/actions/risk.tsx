import { Card, Space, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import {
  AdminReferenceSelect,
  type AdminReferenceOption,
  isReferenceSelectable,
  useAdminReferenceOptions
} from '../../referenceOptions';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import { BooleanSelect, type CreateActionProps, FormModal, type RowActionHelpers, booleanFromSelect, completeCreate, recordString, requiredString, submitAction } from './shared';

type RiskRuleValues = {
  ruleType: string;
  targetType: string;
  targetId: string;
  configJson: string;
  enabled: string;
};

const initialRiskRule: RiskRuleValues = {
  ruleType: '',
  targetType: 'global',
  targetId: '',
  configJson: '{}',
  enabled: 'true'
};

const riskTargetTypeOptions: SemiSelectOption[] = [
  { value: 'global', label: '全局范围' },
  { value: 'user', label: '指定用户' },
  { value: 'pair', label: '指定交易对' },
  { value: 'asset', label: '指定资产' }
];

function riskTargetOptions(
  targetType: string,
  userOptions: AdminReferenceOption[],
  pairOptions: AdminReferenceOption[],
  assetOptions: AdminReferenceOption[]
): AdminReferenceOption[] {
  if (targetType === 'user') {
    return userOptions;
  }
  if (targetType === 'pair') {
    return pairOptions;
  }
  if (targetType === 'asset') {
    return assetOptions.map((option) => ({
      ...option,
      disabled: option.disabled || !option.code,
      disabledReason: option.code ? option.disabledReason : '资产缺少符号，不能作为风控对象',
      value: option.code ?? ''
    }));
  }
  return [];
}

function isRiskRuleCreatable(values: RiskRuleValues, options: AdminReferenceOption[]): boolean {
  const hasTarget = values.targetType === 'global' || isReferenceSelectable(options, values.targetId);
  return Boolean(values.ruleType.trim() && values.targetType.trim() && values.configJson.trim() && hasTarget);
}

export function RiskRuleRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const ruleId = recordString(record, 'id');
  const enabled = record.enabled === true;
  const nextEnabled = !enabled;
  const actionText = enabled ? '禁用' : '启用';

  return (
    <AdminRequestActionBoundary endpoint={`/admin/api/v1/risk/rules/${ruleId}/status`} method="PATCH">
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
    </AdminRequestActionBoundary>
  );
}

export function CreateRiskRuleAction({ onCreated }: CreateActionProps = {}) {
  const [riskRule, setRiskRule] = useState(initialRiskRule);
  const userReferences = useAdminReferenceOptions('user', riskRule.targetType === 'user');
  const pairReferences = useAdminReferenceOptions('marketPair', riskRule.targetType === 'pair');
  const assetReferences = useAdminReferenceOptions('asset', riskRule.targetType === 'asset');
  const targetOptions = riskTargetOptions(riskRule.targetType, userReferences.options, pairReferences.options, assetReferences.options);
  const targetReference = riskRule.targetType === 'user' ? userReferences : riskRule.targetType === 'pair' ? pairReferences : assetReferences;

  return (
    <FormModal actionText="添加风控规则" size="wide" title="添加风控规则">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <label>规则类型<AdminTextInput ariaLabel="规则类型" value={riskRule.ruleType} onChange={(ruleType) => setRiskRule({ ...riskRule, ruleType })} /></label>
            <label>
              对象类型
              <AdminSelect
                ariaLabel="对象类型"
                onChange={(targetType) => setRiskRule({ ...riskRule, targetId: '', targetType })}
                optionList={riskTargetTypeOptions}
                value={riskRule.targetType}
              />
            </label>
            {riskRule.targetType === 'global' ? (
              <label>规则对象<AdminTextInput ariaLabel="规则对象" readOnly value="全局请求（无对象 ID）" onChange={() => undefined} /></label>
            ) : (
              <AdminReferenceSelect
                error={targetReference.error}
                label="规则对象"
                loading={targetReference.loading}
                onChange={(targetId) => setRiskRule({ ...riskRule, targetId })}
                options={targetOptions}
                placeholder="搜索名称、符号或 ID"
                value={riskRule.targetId}
              />
            )}
            <label>规则配置JSON<AdminTextArea ariaLabel="规则配置JSON" autosize value={riskRule.configJson} onChange={(configJson) => setRiskRule({ ...riskRule, configJson })} /></label>
            <label>启用<BooleanSelect label="启用" value={riskRule.enabled} onChange={(enabled) => setRiskRule({ ...riskRule, enabled })} /></label>
          </div>
          <ConfirmAction
            actionText="提交添加风控规则"
            disabled={!isRiskRuleCreatable(riskRule, targetOptions)}
            title="确认添加风控规则"
            onConfirm={async (reason) => {
              if (riskRule.targetType !== 'global' && !isReferenceSelectable(targetOptions, riskRule.targetId)) {
                Toast.error('规则对象已失效或被禁用，请重新选择');
                return;
              }
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
                    target_id: riskRule.targetType === 'global' ? undefined : requiredString(riskRule.targetId, '规则对象'),
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
